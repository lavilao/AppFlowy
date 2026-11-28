// Bridge between the Cactus engine (a static C++ library, Apache-2.0) and
// Dart FFI.
//
// The engine is compiled with a C++ runtime, so it cannot be shipped as an
// Android executable; linking it into this shared library is what makes it
// loadable from Dart. The bridge exists for three reasons:
//
//   1. weights are memory-mapped from a real file, so 17-35 MB of model
//      archive never has to be copied into (or pinned by) the Dart heap;
//   2. the engine transcribes 16 kHz float samples, while Android records
//      16-bit PCM, so the conversion lives here instead of in Dart;
//   3. every entry point is explicitly exported, so Dart can look it up even
//      though the linker only sees it at runtime.
//
// The engine keeps process-global state, so callers must serialize calls
// (CactusEngine does that on the Dart side).

#include <fcntl.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

#include "cactus.h"

#define FLOWY_CACTUS_API __attribute__((visibility("default")))

// The engine keeps pointers into the archive it was given, and it can hold a
// text model and a speech model at the same time (Needle 3 + Whistle), so every
// mapping must outlive every other load. Unmapping on the next load would leave
// the previously loaded model pointing at freed pages — a crash the moment the
// user dictates after extracting to-dos, or the other way round.
//
// Reloading the same path is a no-op: the archive is already mapped and loaded.
#define FLOWY_CACTUS_MAX_MAPPINGS 8

typedef struct {
  char* path;
  void* address;
  size_t size;
} flowy_cactus_mapping;

static flowy_cactus_mapping g_mappings[FLOWY_CACTUS_MAX_MAPPINGS];
static int32_t g_mapping_count = 0;

static flowy_cactus_mapping* find_mapping(const char* path) {
  for (int32_t i = 0; i < g_mapping_count; i++) {
    if (g_mappings[i].path != NULL && strcmp(g_mappings[i].path, path) == 0) {
      return &g_mappings[i];
    }
  }
  return NULL;
}

// Drops every mapping. Only safe once no model is in use, so nothing calls it
// during normal operation; the engine holds the archives for the process life.
FLOWY_CACTUS_API void flowy_cactus_release(void) {
  for (int32_t i = 0; i < g_mapping_count; i++) {
    if (g_mappings[i].address != NULL) {
      munmap(g_mappings[i].address, g_mappings[i].size);
    }
    free(g_mappings[i].path);
    g_mappings[i].path = NULL;
    g_mappings[i].address = NULL;
    g_mappings[i].size = 0;
  }
  g_mapping_count = 0;
}

// Loads a .cact archive from disk. Returns the engine's capability bitmask
// (bit 1 text, bit 2 speech, bit 4 vision) or a negative error code.
FLOWY_CACTUS_API int32_t flowy_cactus_load_file(const char* path) {
  if (path == NULL) {
    return -1;
  }
  if (find_mapping(path) != NULL) {
    // Already mapped and loaded: report what the engine currently holds.
    return (int32_t)cactus_models();
  }
  if (g_mapping_count >= FLOWY_CACTUS_MAX_MAPPINGS) {
    return -5;
  }

  int fd = open(path, O_RDONLY);
  if (fd < 0) {
    return -2;
  }
  struct stat info;
  if (fstat(fd, &info) != 0 || info.st_size <= 0) {
    close(fd);
    return -3;
  }
  size_t size = (size_t)info.st_size;
  void* mapped = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
  close(fd);
  if (mapped == MAP_FAILED) {
    return -4;
  }

  int32_t capabilities =
      (int32_t)cactus_load((const unsigned char*)mapped, (unsigned long long)size);
  if (capabilities < 0) {
    munmap(mapped, size);
    return capabilities;
  }

  char* owned_path = strdup(path);
  if (owned_path == NULL) {
    // The model is loaded but the path cannot be remembered; keep the mapping
    // (returning an error now would leave a loaded model nobody tracks) and
    // let the next call for this path fail its way up instead.
    return capabilities;
  }
  g_mappings[g_mapping_count].path = owned_path;
  g_mappings[g_mapping_count].address = mapped;
  g_mappings[g_mapping_count].size = size;
  g_mapping_count++;
  return capabilities;
}

FLOWY_CACTUS_API int32_t flowy_cactus_models(void) {
  return (int32_t)cactus_models();
}

FLOWY_CACTUS_API const char* flowy_cactus_last_error(void) {
  return cactus_last_error();
}

FLOWY_CACTUS_API void flowy_cactus_set_threads(int32_t threads) {
  cactus_set_threads(threads);
}

// Returns the number of prompt tokens for the toolset, or a negative error.
FLOWY_CACTUS_API int32_t flowy_cactus_init(const char* system_prompt,
                                           const char* tools_json) {
  return (int32_t)cactus_init(system_prompt, tools_json, NULL);
}

FLOWY_CACTUS_API void flowy_cactus_reset(void) { cactus_reset(); }

FLOWY_CACTUS_API int32_t flowy_cactus_complete(const char* input,
                                               int32_t max_new_tokens,
                                               char* out,
                                               int32_t out_capacity) {
  return (int32_t)cactus_complete(input, NULL, 0, max_new_tokens, out,
                                  out_capacity);
}

FLOWY_CACTUS_API int32_t flowy_cactus_transcribe_pcm16(
    const int16_t* pcm,
    int32_t samples,
    const char* language,
    int32_t word_timestamps,
    char* out,
    int32_t out_capacity) {
  if (pcm == NULL || samples <= 0 || out == NULL || out_capacity <= 0) {
    return -1;
  }
  float* samples_f32 = (float*)malloc(sizeof(float) * (size_t)samples);
  if (samples_f32 == NULL) {
    return -2;
  }
  for (int32_t i = 0; i < samples; i++) {
    samples_f32[i] = (float)pcm[i] / 32768.0f;
  }
  int32_t result = (int32_t)cactus_transcribe(samples_f32, samples, language,
                                             NULL, word_timestamps, out,
                                             out_capacity);
  free(samples_f32);
  return result;
}

FLOWY_CACTUS_API int32_t flowy_cactus_describe_rgb(const uint8_t* rgb,
                                                   int32_t width,
                                                   int32_t height,
                                                   const char* options,
                                                   const char* language,
                                                   char* out,
                                                   int32_t out_capacity) {
  if (rgb == NULL || width <= 0 || height <= 0 || out == NULL ||
      out_capacity <= 0) {
    return -1;
  }
  return (int32_t)cactus_describe(rgb, width, height, options, language, NULL,
                                  out, out_capacity);
}

// Returns the embedding dimension when out is NULL, otherwise the number of
// floats written.
FLOWY_CACTUS_API int32_t flowy_cactus_embed(const char* text,
                                            float* out,
                                            int32_t out_capacity) {
  return (int32_t)cactus_embed(text, NULL, 0, out, out_capacity);
}

FLOWY_CACTUS_API const char* flowy_cactus_engine_version(void) {
  return "4.1.4";
}
