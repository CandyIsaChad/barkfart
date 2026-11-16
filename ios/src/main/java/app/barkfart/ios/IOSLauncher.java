package app.barkfart.ios;

import app.barkfart.BarkfartApp;
import com.badlogic.gdx.backends.iosrobovm.IOSApplication;
import com.badlogic.gdx.backends.iosrobovm.IOSApplicationConfiguration;
import org.robovm.apple.foundation.NSAutoreleasePool;
import org.robovm.apple.uikit.UIApplication;

public class IOSLauncher extends IOSApplication.Delegate {
    @Override
    protected IOSApplication createApplication() {
        IOSApplicationConfiguration config = new IOSApplicationConfiguration();
        config.orientationLandscape = true;
        config.orientationPortrait = false;
        // UI-SPEC: strip and stage drags must not open Notification Centre or the home gesture.
        config.screenEdgesDeferringSystemGestures = org.robovm.apple.uikit.UIRectEdge.with(
                org.robovm.apple.uikit.UIRectEdge.Top, org.robovm.apple.uikit.UIRectEdge.Bottom);
        config.hideHomeIndicator = true;
        return new IOSApplication(new BarkfartApp(new IOSPlatform(), 1f), config);
    }

    public static void main(String[] argv) {
        NSAutoreleasePool pool = new NSAutoreleasePool();
        UIApplication.main(argv, null, IOSLauncher.class);
        pool.close();
    }
}
