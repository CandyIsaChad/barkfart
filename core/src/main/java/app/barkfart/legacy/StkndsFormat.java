package app.barkfart.legacy;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/**
 * Legacy project (.stknds), header and stickfigure library only (the frame
 * layout is still being decoded; NOTES.md). Follows the MIT sticknodes-rs
 * spec, with one correction found on real files: in the ID conversion maps,
 * each level stores the container UID first and the count second.
 */
public final class StkndsFormat {
    public static final class Header {
        public int version, build, fps, tweenedFrames;
        public String name;
        public boolean tweening, loop;
        public final List<MovieclipFormat.NamedFigure> figures = new ArrayList<MovieclipFormat.NamedFigure>();
    }

    private StkndsFormat() {}

    /** Reads the header and the stickfigure library from a .stknds file. */
    public static Header readHeader(byte[] file) throws IOException {
        BinIO.In in = new BinIO.In(MovieclipFormat.unpack(file), false);
        Header h = new Header();
        int v = h.version = in.i32();
        if (v < 0 || v > NodesFormat.MAX_VERSION) {
            throw new MovieclipFormat.UnsupportedLayoutException("Stick Nodes project version " + v + " is not supported yet");
        }
        int b = h.build = v >= 401 ? in.i32() : 0;
        h.name = MovieclipFormat.str(in);
        if (v >= 401 && b >= 16) in.u8();                 // vertical (Shorts) video
        if (v >= 244) in.u8();                            // watermark enabled
        if (v >= 220) {
            int n = in.i32();                             // watermark text
            if (n > 0) {
                if (n > in.b.length - in.pos) throw new IOException("bad watermark length");
                in.pos += n;
            }
            in.i32();                                     // watermark colour
        }
        h.fps = in.i32();
        if (v >= 176) h.tweenedFrames = in.i32();
        if (v >= 127) in.u8();                            // shown loop message
        if (v >= 113) in.u8();                            // shown panning message
        in.u8();                                          // shown tweening message
        if (v >= 220) in.u8();                            // shown magnifier message
        h.tweening = in.u8() != 0;
        if (v >= 127) h.loop = in.u8() != 0;
        if (v >= 140 && v < 200) in.u8();
        in.i32();                                         // unique figure id
        if (v >= 176) in.i32();                           // unique textfield id
        if (v >= 300) {
            in.i32();                                     // unique movieclip id
            in.i32();                                     // frames container uid
        }
        if (v >= 400) {
            int fonts = in.count(1);
            in.pos += fonts;
        }
        if (v >= 300) skipInts(in);                       // hidden stickfigure ids
        if (v >= 300) skipInts(in);                       // hidden movieclip ids
        if (v >= 400) skipInts(in);                       // hidden sprite ids
        if (v >= 300) {
            int dest = in.count(8);
            for (int i = 0; i < dest; i++) {
                in.i32();                                 // destination container uid
                int src = in.count(8);
                for (int j = 0; j < src; j++) {
                    in.i32();                             // source container uid
                    int pairs = in.count(8);
                    in.pos += 8 * pairs;
                }
            }
        }
        if (v >= 140 && v < 400) {                        // background image (old versions)
            in.u8();
            if (v < 170) in.u8();
            else {
                in.i32();
                in.i32();
            }
            int w = in.i32(), hh = in.i32();
            long px = (long) w * hh * 3;
            if (w < 0 || hh < 0 || px > in.b.length - in.pos) throw new IOException("bad background image");
            in.pos += (int) px;
        }
        int figs = in.count(8);
        for (int i = 0; i < figs; i++) {
            MovieclipFormat.NamedFigure nf = new MovieclipFormat.NamedFigure();
            nf.name = MovieclipFormat.str(in);
            nf.figure = NodesFormat.readEmbedded(in);
            h.figures.add(nf);
        }
        return h;
    }

    private static void skipInts(BinIO.In in) throws IOException {
        int n = in.count(4);
        in.pos += 4 * n;
    }
}
