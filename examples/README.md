# Demo scene

`flytest-demo.blend` is a small headless-generated Blender 5.2 scene. It contains the `FlyTest` collection, a simple arena, fly body, wings, eyes, camera, key light, and baked transform keyframes.

Open it in Blender or regenerate a longer scene:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts/run_demo.ps1 `
  -Ticks 600 `
  -WithBlender `
  -Blender "C:\Program Files\Blender Foundation\Blender 5.2\blender.exe"
```
