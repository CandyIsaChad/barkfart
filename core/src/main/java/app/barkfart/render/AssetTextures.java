package app.barkfart.render;

import app.barkfart.model.Project;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.utils.Disposable;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/** Decodes image assets of a project into GL textures on first use; caches them. */
public final class AssetTextures implements MeshRenderer.Textures, Disposable {
    private final Project project;
    private final Map<String, Texture> cache = new HashMap<String, Texture>();
    private final Set<String> failed = new HashSet<String>();

    public AssetTextures(Project project) {
        this.project = project;
    }

    public Texture get(String assetId) {
        Texture t = cache.get(assetId);
        if (t != null || failed.contains(assetId)) return t;
        try {
            byte[] b = project.assetData.bytes(assetId);
            Pixmap pm = new Pixmap(b, 0, b.length);
            t = new Texture(pm);
            t.setFilter(Texture.TextureFilter.Linear, Texture.TextureFilter.Linear);
            pm.dispose();
            cache.put(assetId, t);
        } catch (Exception e) {
            failed.add(assetId);   // drawn as plain colour; reported once
            Gdx.app.error("Barkfart", "image " + assetId + " could not be loaded: " + e.getMessage());
        }
        return t;
    }

    /** Forget one asset (replaced or removed). */
    public void invalidate(String assetId) {
        Texture t = cache.remove(assetId);
        if (t != null) t.dispose();
        failed.remove(assetId);
    }

    @Override
    public void dispose() {
        for (Texture t : cache.values()) t.dispose();
        cache.clear();
    }
}
