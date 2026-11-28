# Cactus models

Place the Cactus model archives here before building. They are downloaded
during the build instead of being committed (74 MB total), and each one is
verified against a pinned SHA-256:

| File | Size | SHA-256 |
|---|---|---|
| `whistle.cact` | 16919407 | `b6e02f048568ac5d01a2042556c658061e699acbc0aa2a1439f52f3d461dffeb` |
| `mirage.cact`  | 21611043 | `4249a0f7515272ff76b9414bec17d7e7daf84b15b66c7fb4f17fce1e66f9f237` |
| `needle3.cact` | 35335380 | `c9d915eca282ed42d1a09b143b592adb4cc6744ffe2d294adf5cfc5548170c38` |

All three run on the single Cactus engine (Apache-2.0), which is statically
linked from the `android-armv7` / `android-arm64` release archives of
`Cactus-Compute/cactus` at tag `v4.1.4`:

- https://huggingface.co/Cactus-Compute/whistle/resolve/main/whistle.cact
- https://huggingface.co/Cactus-Compute/mirage/resolve/main/mirage.cact
- https://huggingface.co/Cactus-Compute/needle3/resolve/main/needle3.cact
