# Video dithering

This program is inspired by the Matt Parker video ["The magic of adding random
noise to black and white images."](https://www.youtube.com/watch?v=kT4p1GXq4HY)

The idea I got is that with Bayer dithering we were using the spacial component
for showing the grey levels.  While with the random or blue noise, we were
actually using the temporal component.  Each pixel was lighting up and getting
darker irrespective of the luminosity of it's neighbours.

I was wondering if the randomness and the selection of the spatial/temporal axis
is really connected.  Can we do non-random pattern on the temporal axis?

This program takes a sequence of PNG images and treats them as frames of a
video, converting individual "cells" into shades of grey.

Each cell consists of 8 pixels: a 2 by 2 grid in one frame, and the same 2 by 2
grid in the subsequent frame.  We take these 8 pixels, compute their combined
luminosity and replace them all with a predefined pattern, that is supposed to
use the fact that we have 2 frames to provide 8 levels of gray, compared to only
2 you get with Bayer dithering of 2x2.

Right now, there is a noticeable flickering caused by the synchronization of the
temporal patterns across frames.  I think this is a similar problem that the
Bayer pattern solves for the spacial repetition.  But it might require a
slightly different solution for the temporal axis.

There could probably be other interesting patterns.

I wonder if there is a way to apply [Floyd–Steinberg
dithering](https://en.wikipedia.org/wiki/Floyd%E2%80%93Steinberg_dithering) to
the temporal component.  It might flicker way less.

## How to use

Extract frames as separate images into a folder, starting at time specified by
`-ss` and output `-vframes` into the folder:

```sh
mkdir -p input
ffmpeg -ss 0:0:0 -i input.mkv -vframes 1000 input/%06d.png
```

You can use BMP, to speed up processing, as PNG encoding/decoding takes time.
But note that a minute of 4K video at 25 fps takes about 30GBs in BMP.

Now run the filter:

```sh
mkdir -p output
cargo run --release -- --input-template 'input/{frame_nr}.png' --output output/
```

Check the input frame rate:

```sh
ffprobe -v quiet -print_format default=noprint_wrappers=1:nokey=1 \
  -show_entries stream=r_frame_rate \
  -select_streams v:0 \
  input.mkv
```

Which would output something like `25/1` or `24000/1001`.

Watch the produced video:

```sh
mpv -mf-fps '[input frame rate]' 'mf://output/*.png'
```

(Note that you need to quote the glob (`*`) in the path, in order for `mpv` to
see it.  Otherwise your shell will insert all the frame names as individual
arguments, which could overflow the process argument list if there are too many
frames.)

Replace video stream in the original video with the processed version:

```sh
ffmpeg -i input.mkv -framerate '[input frame rate]' -i output/%06d.png \
  -map 1:v -map 0 -map -0:v \
  -c copy -c:v libx265 -crf 18 \
  -shortest \
  output.mkv
```

Note that some metadata from the original video stream might be lost.  Such as:

* Color range (limited vs. full)
* Sample aspect ratio / DAR

`ffprobe` can be used to extract it, and additional flags would be needed to
include it in the final encoding step.

## References

* [WebGPU Spec](https://gpuweb.github.io/gpuweb/)
