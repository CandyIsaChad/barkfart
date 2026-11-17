package app.barkfart.render;

import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.glutils.FrameBuffer;
import com.badlogic.gdx.utils.Disposable;
import java.util.ArrayList;
import java.util.List;

/**
 * Pooled offscreen buffers for the effects pipeline (DESIGN §5). Sizes are
 * rounded up to 64 px so buffers get reused as groups move and resize.
 */
public final class BufferPool implements Disposable {
    private static final int STEP = 64;
    private final List<FrameBuffer> free = new ArrayList<FrameBuffer>();
    private final List<FrameBuffer> all = new ArrayList<FrameBuffer>();

    static int round(int px) {
        return Math.max(STEP, (px + STEP - 1) / STEP * STEP);
    }

    /** A buffer at least w x h pixels (exactly the rounded size). Release it after use. */
    public FrameBuffer acquire(int w, int h) {
        int rw = round(w), rh = round(h);
        for (int i = 0; i < free.size(); i++) {
            FrameBuffer fb = free.get(i);
            if (fb.getWidth() == rw && fb.getHeight() == rh) return free.remove(i);
        }
        FrameBuffer fb = new FrameBuffer(Pixmap.Format.RGBA8888, rw, rh, false);
        all.add(fb);
        return fb;
    }

    public void release(FrameBuffer fb) {
        if (fb != null && !free.contains(fb)) free.add(fb);
    }

    /** Frees buffers not used since the last trim (call when memory matters). */
    public void trim() {
        for (FrameBuffer fb : free) {
            all.remove(fb);
            fb.dispose();
        }
        free.clear();
    }

    public int allocated() {
        return all.size();
    }

    @Override
    public void dispose() {
        for (FrameBuffer fb : all) fb.dispose();
        all.clear();
        free.clear();
    }
}
