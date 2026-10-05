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

**Lucide icons** - `crates/reco-desktop/resources/icons/*.svg` are
[Lucide](https://lucide.dev) icons (npm `lucide-static` 1.52.0, fetched by
`crates/reco-desktop/tools/lucide_icons.py`), recoloured to black for tinting,
with an invisible box added, and `circle`/`square` filled for Record and
Stop. ISC License, Copyright (c) 2026 Lucide Icons and Contributors:

> Permission to use, copy, modify, and/or distribute this software for any
> purpose with or without fee is hereby granted, provided that the above
> copyright notice and this permission notice appear in all copies.
>
> THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
> WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
> MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
> ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
> WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
> ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
> OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

Lucide's `check`, `chevron-down`, `circle`, `plus`, `square` and `x` derive
from [Feather](https://github.com/feathericons/feather), MIT License,
Copyright (c) 2013-present Cole Bemis:

> Permission is hereby granted, free of charge, to any person obtaining a
> copy of this software and associated documentation files (the
> "Software"), to deal in the Software without restriction, including
> without limitation the rights to use, copy, modify, merge, publish,
> distribute, sublicense, and/or sell copies of the Software, and to permit
> persons to whom the Software is furnished to do so, subject to the
> following conditions:
>
> The above copyright notice and this permission notice shall be included
> in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
> OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
> MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
> NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
> OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
> USE OR OTHER DEALINGS IN THE SOFTWARE.

**Inter** - `reco-desktop` sets its text in Inter, loaded from the copy
`makepad-widgets` ships (`widgets/resources/Inter.ttf`), SIL Open Font
License 1.1, Copyright 2020 The Inter Project Authors.

**Rerun** - `reco-desktop`'s look follows the Rerun viewer's design system
(`re_ui`: its grey scale, 12 px Inter, 24 px rows and time panel). No Rerun
code or assets are copied. Rerun is MIT OR Apache-2.0, Copyright (c) Rerun
Technologies AB.
