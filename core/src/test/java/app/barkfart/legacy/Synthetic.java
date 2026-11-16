package app.barkfart.legacy;

/** Synthetic legacy figures built in code (committable fixtures, no original assets). */
final class Synthetic {
    private Synthetic() {}

    static NodesFormat.LegacyFigure figure(int version, int build) {
        NodesFormat.LegacyFigure fig = new NodesFormat.LegacyFigure();
        fig.version = version;
        fig.build = version >= 403 ? build : 0;
        fig.scale = 1f;
        fig.color = 0xff202020;
        fig.littleEndian = version < 160;
        fig.compressed = version < 160;
        fig.root = node(fig, -1, 0, 0, 0, 0, 0);
        NodesFormat.LegacyNode torso = node(fig, 0, 1, 0, 45, 90, 90);
        fig.root.children.add(torso);
        NodesFormat.LegacyNode head = node(fig, 4, 2, 0, 30, 60, 0);
        torso.children.add(head);
        NodesFormat.LegacyNode arm = node(fig, 0, 3, -30, 0, 60, 90);
        torso.children.add(arm);
        // Offsets must agree with the angles (world = sum of local angles), at 0.5x length.
        double legWorld = Math.toRadians(-158);
        NodesFormat.LegacyNode leg = node(fig, 1, 4, (float) (54 * Math.cos(legWorld)), (float) (54 * Math.sin(legWorld)), 108, -158);
        fig.root.children.add(leg);
        if (version >= 230) {
            NodesFormat.Polyfill p = new NodesFormat.Polyfill();
            p.anchor = 1;
            p.color = 0xff0000ff;
            p.useColor = 1;
            p.nodes.add(2);
            p.nodes.add(3);
            fig.polyfills.add(p);
        }
        return fig;
    }

    static NodesFormat.LegacyNode node(NodesFormat.LegacyFigure fig, int type, int draw, float lx, float ly, float len, float angle) {
        NodesFormat.LegacyNode n = new NodesFormat.LegacyNode();
        for (NodesFormat.Field f : NodesFormat.NODE) {
            if (!f.present(fig.version, fig.build)) continue;
            n.fields.put(f.name, f.kind == NodesFormat.Kind.F32 ? (Object) 0f : (Object) 0);
        }
        n.fields.put("type", type);
        n.fields.put("drawIndex", draw);
        n.fields.put("localX", lx);
        n.fields.put("localY", ly);
        n.fields.put("scale", 1f);
        n.fields.put("defaultLength", len);
        n.fields.put("length", len);
        n.fields.put("defaultThickness", 21);
        n.fields.put("thickness", 21);
        n.fields.put("localAngle", angle);
        n.fields.put("color", 0xff1f1f1f);
        return n;
    }
}
