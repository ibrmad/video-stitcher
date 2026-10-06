# Quick Start Guide

## 1. Download & Open

Download the latest **reco-desktop** release for your platform from the [Releases page](https://github.com/reco-project/video-stitcher/releases). Also download `yolo26n.onnx` from the same release (needed for AI tracking).

Extract it and open **Reco.app** (macOS) or run `reco-desktop` (Windows, Linux). Keep the files beside it: they hold its fonts, icons and libraries.

- **Windows**: If SmartScreen blocks the app, right-click the exe > Properties > check "Unblock" > OK, then run again.
- **Mac**: The app isn't notarized. The first time, open it, then allow it in System Settings > Privacy & Security > **Open Anyway**.

For AI tracking, also download `yolo26n.onnx` (1280, higher accuracy) or `yolo26n_640.onnx` (640, faster on integrated GPUs) from the same release.

## 2. Import & Calibrate

- In the **Setup** panel (left), click **Add…** beside **Left camera** and pick that camera's file(s); do the same for **Right camera**. Dropping files on the window works too
- Under **Calibration**, click **Auto-calibrate** and wait for it to finish

Your stitched panorama should appear in the preview. Pan with mouse drag, zoom with the scroll wheel.

## 3. Fix the Lens (if needed)

If the image looks warped (curved lines that should be straight):

- Open the **Adjust** panel (right; ⌘2 / Ctrl+2)
- Under **Lens**, click **Lens profiles**, search for your camera (e.g. `hero9 wide`) and pick it
- **Show** one camera on its own to check that straight lines look straight
- Click **Recalibrate** in Setup > Calibration: it matches the cameras again with the new lens and saves the result

## 4. Set the Field Area

- In Setup > Calibration, click **Edit in browser…** beside **Field outline** - a browser page opens after a few seconds
- Draw a polygon covering the field visible in the **left camera**
- Switch to **Right Camera** in the dropdown and do the same for the right
- Click **Copy ROI**, go back to the app, paste it into **Paste the outline here** and click **Use**

## 5. Export

- Click **Export** in the top bar
- Pick the file (**Save to…**), the **Size** (1080p recommended), and the **Start** and **End** (the timeline shows the range tinted)
- To enable AI tracking:
  - Tick **Follow the play**
  - **Choose…** the `yolo26n.onnx` model you downloaded
  - Set **Follow** to **Players and ball** and **Detection** to **Every 15 frames**
- Click **Export**

## 6. Debug AI Tracking (optional)

To see what the AI detected, export pipeline events and visualize them:

```bash
reco stitch left.mp4 right.mp4 -c cal.json --model yolo26n.onnx \
    --tracking field --detection-interval 15 --events detections.jsonl -o output.mp4

python3 scripts/visualize_detections.py export detections.jsonl left.mp4 right.mp4 \
    -c cal.json -o annotated.mp4
```

This produces a side-by-side video with detection boxes, ROI boundaries, tracking state, and panner decisions overlaid.

## Tips

- Try a short clip first (set Start and End) before exporting the full video
- For football, use **Players and ball** with detection every **10-15** frames for smooth panning
- Use **Sync (frames)** under Adjust > **Stitch** to fine-tune camera alignment after calibration
- Share your calibration file with us if something looks wrong
- Can't find a lens profile for your camera? Let us know
- Join the community at [forum.reco-project.org](https://forum.reco-project.org)
