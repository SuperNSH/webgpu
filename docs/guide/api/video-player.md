# Video player

Unsupported.

`createVideoPlayer(source, options?)` exists and throws `ERR_WEBGPU_UNSUPPORTED`. In react-native-webgpu it decodes a video with the platform's player (`AVPlayer`, `MediaPlayer`) into `NativeBuffer` frames, for tests and examples. Here it would need a platform decoder and zero-copy import of its frames, and wgpu has no shared texture memory ([NativeBuffer](native-buffer.md)).

The types are kept as the reference declares them: `VideoPlayer` (`copyLatestFrame()`, `play()`, `pause()`, `release()`, `paused`, `currentTime`, `duration`, `volume`, `loop`, `videoWidth`, `videoHeight`, `rotation`, `frameRate`) and `CreateVideoPlayerOptions` (`pixelFormat`).

For a moving test image, `createTestVideoFrame` gives the reference's patterns in memory, to upload with `queue.writeTexture()` ([NativeBuffer](native-buffer.md#createtestvideoframe)).
