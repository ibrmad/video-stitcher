# Third-Party Notices

## Acknowledgments

**Gyroflow** - The CUDA/Vulkan GPU interop architecture was inspired by
[Gyroflow](https://github.com/gyroflow/gyroflow)'s approach to zero-copy
GPU frame sharing. All implementations in reco are original.

**Gyroflow Lens Profiles** - The lens profile database is converted from
the [Gyroflow lens_profiles](https://github.com/gyroflow/lens_profiles)
repository, which is released under CC0-1.0 (public domain).

**telemetry-parser** - IMU telemetry extraction uses the
[telemetry-parser](https://github.com/AdrianEddy/telemetry-parser) crate
by Adrian Eddy (MIT OR Apache-2.0).

**AKAZE** - Feature detection is based on
[Christopher22/akaze](https://github.com/Christopher22/akaze) (MIT),
with local bug fixes.

**Lens Distortion** - The Kannala-Brandt fisheye model follows the
published paper and OpenCV documentation.

**Makepad icons** - `crates/reco-desktop/resources/icons/{help,play,pause,plus,folder,check}.svg`
are copied from [Makepad](https://github.com/makepad/makepad)
(`libs/fab/resources/icons`), MIT License, Copyright (c) 2023 Makepad B.V.

**Inter** - `reco-desktop` sets its text in Inter, loaded from the copy
`makepad-widgets` ships (`widgets/resources/Inter.ttf`), SIL Open Font
License 1.1, Copyright 2020 The Inter Project Authors.

**Rerun** - `reco-desktop`'s look follows the Rerun viewer's design system
(`re_ui`: its grey scale, 12 px Inter, 24 px rows and time panel). No Rerun
code or assets are copied. Rerun is MIT OR Apache-2.0, Copyright (c) Rerun
Technologies AB.
