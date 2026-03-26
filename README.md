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
