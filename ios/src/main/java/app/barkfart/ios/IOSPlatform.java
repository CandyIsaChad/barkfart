package app.barkfart.ios;

import app.barkfart.platform.Platform;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.iosrobovm.IOSApplication;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.robovm.apple.foundation.NSArray;
import org.robovm.apple.foundation.NSFileManager;
import org.robovm.apple.foundation.NSObject;
import org.robovm.apple.foundation.NSSearchPathDirectory;
import org.robovm.apple.foundation.NSSearchPathDomainMask;
import org.robovm.apple.foundation.NSURL;
import org.robovm.apple.uikit.UIActivityViewController;
import org.robovm.apple.uikit.UIDocumentPickerDelegateAdapter;
import org.robovm.apple.uikit.UIDocumentPickerMode;
import org.robovm.apple.uikit.UIDocumentPickerViewController;
import org.robovm.apple.uikit.UIPopoverPresentationController;
import org.robovm.apple.uikit.UIView;
import org.robovm.apple.uikit.UIViewController;
import org.robovm.apple.coregraphics.CGRect;

/** iOS: Documents folder, system document picker, share sheet. */
final class IOSPlatform implements Platform {
    private UIDocumentPickerDelegateAdapter delegate;   // keep a strong reference while presented

    public File documents() {
        List<NSURL> urls = NSFileManager.getDefaultManager().getURLsForDirectory(
                NSSearchPathDirectory.DocumentDirectory, NSSearchPathDomainMask.UserDomainMask);
        return new File(urls.get(0).getPath());
    }

    public boolean canPick() {
        return true;
    }

    private UIViewController root() {
        return ((IOSApplication) Gdx.app).getUIViewController();
    }

    public void pickFiles(final PickCallback cb) {
        UIDocumentPickerViewController picker = new UIDocumentPickerViewController(
                Arrays.asList("public.data", "public.item"), UIDocumentPickerMode.Import);
        picker.setAllowsMultipleSelection(true);
        delegate = new UIDocumentPickerDelegateAdapter() {
            @Override
            public void didPickDocuments(UIDocumentPickerViewController controller, NSArray<NSURL> urls) {
                final List<Picked> out = new ArrayList<Picked>();
                for (NSURL u : urls) {
                    try {
                        boolean scoped = u.startAccessingSecurityScopedResource();
                        try {
                            File f = new File(u.getPath());
                            out.add(new Picked(f.getName(), readAll(f)));
                        } finally {
                            if (scoped) u.stopAccessingSecurityScopedResource();
                        }
                    } catch (Exception e) {
                        Gdx.app.error("Barkfart", "could not read " + u, e);
                    }
                }
                Gdx.app.postRunnable(new Runnable() {
                    public void run() {
                        cb.picked(out);
                    }
                });
            }

            @Override
            public void wasCancelled(UIDocumentPickerViewController controller) {
                Gdx.app.postRunnable(new Runnable() {
                    public void run() {
                        cb.picked(new ArrayList<Picked>());
                    }
                });
            }
        };
        picker.setDelegate(delegate);
        root().presentViewController(picker, true, null);
    }

    private IOSAudio audio;

    public app.barkfart.audio.AudioBackend audio() {
        if (audio == null) audio = new IOSAudio();
        return audio;
    }

    public String verifyVideo(File f, int w, int h, double seconds, boolean audio) {
        try {
            if (!f.exists() || f.length() == 0) return "no file written";
            org.robovm.apple.avfoundation.AVURLAsset a = new org.robovm.apple.avfoundation.AVURLAsset(new NSURL(f), null);
            NSArray<org.robovm.apple.avfoundation.AVAssetTrack> v = a.getTracksWithType(org.robovm.apple.avfoundation.AVMediaType.Video);
            if (v == null || v.size() == 0) return "no video track";
            org.robovm.apple.coregraphics.CGSize sz = v.get(0).getNaturalSize();
            if (Math.round(sz.getWidth()) != w || Math.round(sz.getHeight()) != h) return "video is " + sz.getWidth() + "x" + sz.getHeight() + ", expected " + w + "x" + h;
            double d = a.getDuration().getSeconds();
            if (Math.abs(d - seconds) > 0.25) return "duration " + d + " s, expected " + seconds + " s";
            NSArray<org.robovm.apple.avfoundation.AVAssetTrack> au = a.getTracksWithType(org.robovm.apple.avfoundation.AVMediaType.Audio);
            if (audio && (au == null || au.size() == 0)) return "no audio track";
            return null;
        } catch (Exception e) {
            return String.valueOf(e);
        }
    }

    public boolean canEncodeVideo() {
        return true;
    }

    public app.barkfart.export.VideoSink openVideo(File out, int w, int h, double fps, File wav) {
        String[] why = new String[1];
        app.barkfart.export.VideoSink v = IOSVideo.open(out, w, h, fps, wav, why);
        if (v == null) Gdx.app.error("Barkfart", "video: " + why[0]);
        return v;
    }

    public boolean canShare() {
        return true;
    }

    public void share(File f) {
        NSURL url = new NSURL(f);
        NSArray<NSObject> items = new NSArray<NSObject>(url);
        UIActivityViewController share = new UIActivityViewController(items, null);
        UIViewController root = root();
        // iPad: a share sheet must be anchored or iOS throws.
        UIPopoverPresentationController pop = share.getPopoverPresentationController();
        if (pop != null) {
            UIView v = root.getView();
            pop.setSourceView(v);
            CGRect b = v.getBounds();
            pop.setSourceRect(new CGRect(b.getWidth() / 2, b.getHeight() / 2, 1, 1));
        }
        root.presentViewController(share, true, null);
    }

    private static byte[] readAll(File f) throws java.io.IOException {
        FileInputStream in = new FileInputStream(f);
        try {
            ByteArrayOutputStream o = new ByteArrayOutputStream();
            byte[] b = new byte[65536];
            int n;
            while ((n = in.read(b)) > 0) o.write(b, 0, n);
            return o.toByteArray();
        } finally {
            in.close();
        }
    }
}
