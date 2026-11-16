# Third-party notices

## sticknodes-rs

The legacy .nodes field schema in core/src/main/java/app/barkfart/legacy/NodesFormat.java
follows the reader and specification of sticknodes-rs
(https://github.com/vinceTheProgrammer/sticknodes-rs, commit 3ea03d6), used under the MIT License:

```
MIT License

Copyright (c) 2025 vinceTheProgrammer

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Inter typeface

assets/fonts/Inter-Regular.ttf and Inter-SemiBold.ttf are from Inter 4.1 by
Rasmus Andersson (https://github.com/rsms/inter), used under the SIL Open
Font License 1.1; the full licence is in assets/fonts/Inter-LICENSE.txt.

## gdx-freetype (dependency)

LibGDX's FreeType extension (Apache 2.0, same project as LibGDX) renders the
TTF fonts at the device's pixel density. Added because LibGDX's bundled
bitmap font blurs when scaled on high-density screens.
