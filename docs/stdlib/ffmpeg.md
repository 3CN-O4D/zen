# ffmpeg — media conversion

ffmpeg wrappers for probing and converting media. Requires the `ffmpeg`
binary on `PATH` (`available()` reports whether it is present).

```zen
import ffmpeg

print ffmpeg.available()
```

## Functions

| Function | Description |
|----------|-------------|
| `available()` | Whether the `ffmpeg` binary is reachable. |
| `probe(path)` | Media metadata dict. |
| `duration_ms(path)` | Duration in milliseconds. |
| `thumbnail(src, out, size)` | Still image thumbnail. |
| `image(src, out)` | Frame extraction. |
| `to_webp(src, out)` / `sticker(src, out)` | WebP conversion. |
| `video_to_gif(src, out, fps)` | GIF conversion. |
| `mp3(src, out)` / `audio(src, out, mime)` | Audio extraction. |
| `trim(src, out, start, duration)` | Segment cutting. |
| `resize(src, out, w, h)` | Frame resizing. |
| `to_mp4(src, out)` | MP4 conversion. |
| `merge(out, inputs)` | Concatenation. |

## Example

```zen
import ffmpeg

if ffmpeg.available() {
    print ffmpeg.duration_ms("clip.mp4")
    ffmpeg.thumbnail("clip.mp4", "thumb.jpg", "320x240")
}
```
