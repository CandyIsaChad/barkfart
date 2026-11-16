package app.barkfart.ui;

import app.barkfart.demo.Demo;
import app.barkfart.edit.KeyRef;
import app.barkfart.edit.StructureEdits;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.ui.Label;
import com.badlogic.gdx.scenes.scene2d.ui.ScrollPane;
import com.badlogic.gdx.scenes.scene2d.ui.Table;
import com.badlogic.gdx.scenes.scene2d.ui.WidgetGroup;
import com.badlogic.gdx.utils.Align;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Left sidebar (UI-SPEC "Sidebar"). Rebuilt from EditorState on every change. */
public final class Sidebar extends WidgetGroup {
    private final Theme theme;
    private final EditorState st;
    private final UiButton undo, redo, modeFigure, modeAll, modeCamera;
    private final Label readout, status;
    private final Table panel = new Table();
    private final ScrollPane scroll;
    private Map<String, float[]> copiedPose;
    private float insetL, insetB;
    private float builtWidth = -1;

    /** Safe-area insets: the panel runs under them, controls stay inside. */
    public void setPadding(float left, float bottom) {
        insetL = left;
        insetB = bottom;
    }

    public Sidebar(Theme theme, EditorState st) {
        this.theme = theme;
        this.st = st;
        undo = new UiButton(theme, "Undo", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.doc.undo();
            }
        });
        redo = new UiButton(theme, "Redo", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.doc.redo();
            }
        });
        modeFigure = mode("Figure", EditorState.Mode.FIGURE);
        modeAll = mode("All", EditorState.Mode.ALL);
        modeCamera = mode("Camera", EditorState.Mode.CAMERA);
        readout = new Label("", new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        status = new Label("", new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        status.setEllipsis(true);
        status.addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent event, float x, float y, int pointer, int button) {
                Runnable a = st.messageAction();
                if (a != null) a.run();
                return a != null;
            }
        });
        panel.top().left();
        scroll = new ScrollPane(panel);
        scroll.setScrollingDisabled(true, false);
        scroll.setFadeScrollBars(true);
        scroll.setOverscroll(false, true);
        for (Actor a : new Actor[] {undo, redo, modeFigure, modeAll, modeCamera, readout, scroll, status}) addActor(a);
        st.addListener(new Runnable() {
            public void run() {
                refresh();
            }
        });
        refresh();
    }

    private UiButton mode(String text, final EditorState.Mode m) {
        return new UiButton(theme, text, UiButton.Style.SEGMENT_SMALL, new Runnable() {
            public void run() {
                st.mode = m;
                st.fire();
            }
        });
    }

    @Override
    public void layout() {
        float w = getWidth() - insetL, h = getHeight(), p = Theme.PAD + insetL, g = Theme.GAP;
        float inner = w - 2 * Theme.PAD;
        float bw = (inner - g) / 2;
        float y = h - p - Theme.CONTROL_H;
        undo.setBounds(p, y, bw, Theme.CONTROL_H);
        redo.setBounds(p + bw + g, y, bw, Theme.CONTROL_H);
        y -= 20;
        readout.setBounds(p, y, inner, 16);
        y -= 4 + Theme.CONTROL_H;
        float sw = (inner - 2 * 2) / 3;
        modeFigure.setBounds(p, y, sw, Theme.CONTROL_H);
        modeAll.setBounds(p + sw + 2, y, sw, Theme.CONTROL_H);
        modeCamera.setBounds(p + 2 * (sw + 2), y, sw, Theme.CONTROL_H);
        status.setBounds(p, 6 + insetB, inner, 16);
        float top = y - g, bottom = 6 + insetB + 16 + g;
        scroll.setBounds(p, bottom, inner, Math.max(0, top - bottom));
        if (inner != builtWidth) {
            builtWidth = inner;
            rebuildPanel();
        }
        panel.invalidateHierarchy();
    }

    @Override
    public void draw(Batch batch, float parentAlpha) {
        Color c = batch.getColor().cpy();
        batch.setColor(Theme.PANEL);
        batch.draw(theme.white(), getX(), getY(), getWidth(), getHeight());
        batch.setColor(c);
        super.draw(batch, parentAlpha);
    }

    private void refresh() {
        undo.enabled = st.doc.canUndo() && !st.playing;
        redo.enabled = st.doc.canRedo() && !st.playing;
        modeFigure.selected = st.mode == EditorState.Mode.FIGURE;
        modeAll.selected = st.mode == EditorState.Mode.ALL;
        modeCamera.selected = st.mode == EditorState.Mode.CAMERA;
        String modeName = st.mode == EditorState.Mode.CAMERA ? "Camera" : (st.mode == EditorState.Mode.ALL ? "All" : "Figure");
        readout.setText(modeName + " · " + Math.round(st.zoom * 100) + "%");
        String msg = st.message();
        if (msg != null) {
            status.setText(msg);
            status.getStyle().fontColor = Theme.TEXT;
        } else {
            int f = st.playing ? st.playingFrame() : st.frame;
            status.setText("Frame " + (f + 1) + " / " + st.frames.frameCount() + " · " + String.format("%.2f s", st.frames.time(f)));
            status.getStyle().fontColor = Theme.TEXT_DIM;
        }
        rebuildPanel();
    }

    private Label title(String s) {
        Label l = new Label(s, new Label.LabelStyle(theme.title, Theme.TEXT));
        l.setEllipsis("…");
        return l;
    }

    private Label note(String s) {
        Label l = new Label(s, new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        l.setWrap(true);
        return l;
    }

    private float innerWidth() {
        return Math.max(0, getWidth() - insetL - 2 * Theme.PAD);
    }

    private void row(UiButton a, UiButton b) {
        float half = (innerWidth() - Theme.GAP) / 2;
        panel.add(a).width(half).height(Theme.CONTROL_H).padBottom(Theme.GAP);
        panel.add(b).width(half).height(Theme.CONTROL_H).padBottom(Theme.GAP).padLeft(Theme.GAP).row();
    }

    private void full(Actor a) {
        panel.add(a).colspan(2).width(innerWidth()).height(Theme.CONTROL_H).padBottom(Theme.GAP).row();
    }

    private void rebuildPanel() {
        panel.clearChildren();
        panel.defaults().left();
        float innerW = innerWidth();
        if (st.playing) {
            panel.add(note("Playing. Tap Stop or the stage to return to frame " + (st.frame + 1) + "."))
                    .colspan(2).width(innerW).row();
            return;
        }
        if (!st.selectedKeys.isEmpty()) {
            keyPanel(innerW);
            return;
        }
        if (st.selectedLayer == null) {
            panel.add(title("Nothing selected")).colspan(2).width(innerW).padBottom(Theme.GAP).row();
            full(new UiButton(theme, "Add figure", UiButton.Style.ACCENT_OUTLINE, new Runnable() {
                public void run() {
                    addFigure();
                }
            }));
            UiButton onion = new UiButton(theme, "Onion skin", UiButton.Style.SEGMENT, new Runnable() {
                public void run() {
                    st.onion = !st.onion;
                    st.fire();
                }
            });
            onion.selected = st.onion;
            full(onion);
            panel.add(note("Frame rate: " + Math.round(st.editor.fps()) + " fps")).colspan(2).width(innerW).padBottom(4).row();
            row(new UiButton(theme, "−", UiButton.Style.NORMAL, fps(-1)), new UiButton(theme, "+", UiButton.Style.NORMAL, fps(1)));
            return;
        }
        Layer.FigureLayer l = st.selectedLayer;
        String what = st.selectedNode == null ? l.name
                : (st.selectedNode.parent == null ? l.name + " · main node" : "Joint " + jointNumber(l, st.selectedNode));
        panel.add(title(what)).colspan(2).width(innerW).padBottom(Theme.GAP).row();
        row(new UiButton(theme, "Previous", UiButton.Style.NORMAL, cycle(-1)), new UiButton(theme, "Next", UiButton.Style.NORMAL, cycle(1)));
        row(new UiButton(theme, "Copy pose", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                copyPose();
            }
        }), pasteButton());
        UiButton rotate = new UiButton(theme, "Rotate", UiButton.Style.SEGMENT, stretch(false));
        UiButton stretch = new UiButton(theme, "Stretch", UiButton.Style.SEGMENT, stretch(true));
        rotate.selected = !st.tools.stretch;
        stretch.selected = st.tools.stretch;
        panel.add(note("Drag")).colspan(2).width(innerW).padBottom(4).row();
        row(rotate, stretch);
        if (st.selectedNode != null && st.selectedNode.parent != null) {
            panel.add(note("Angle")).colspan(2).width(innerW).padBottom(4).row();
            panel.add(new AngleScrub(theme, st, st.selectedNode)).colspan(2).width(innerW).height(Theme.CONTROL_H)
                    .padBottom(Theme.GAP).row();
        }
        full(new UiButton(theme, "Delete figure", UiButton.Style.DANGER, new Runnable() {
            public void run() {
                deleteFigure();
            }
        }));
    }

    private List<KeyRef> sel() {
        return new ArrayList<KeyRef>(st.selectedKeys);
    }

    private void keyPanel(float innerW) {
        int n = st.selectedKeys.size();
        panel.add(title(n == 1 ? "1 key" : n + " keys")).colspan(2).width(innerW).padBottom(Theme.GAP).row();
        row(new UiButton(theme, "Copy", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.keyClipboard = st.editor.copy(sel());
                st.flash("Copied " + st.selectedKeys.size() + " keys", null);
            }
        }), pasteKeysButton());
        panel.add(note("Easing")).colspan(2).width(innerW).padBottom(4).row();
        row(ease("Hold", app.barkfart.model.Interp.HOLD, null), ease("Linear", app.barkfart.model.Interp.LINEAR, null));
        row(ease("Ease", app.barkfart.model.Interp.BEZIER, app.barkfart.model.Ease.EASE_IN_OUT),
                ease("In", app.barkfart.model.Interp.BEZIER, app.barkfart.model.Ease.EASE_IN));
        row(ease("Out", app.barkfart.model.Interp.BEZIER, app.barkfart.model.Ease.EASE_OUT), new UiButton(theme, "Clear", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.selectedKeys.clear();
                st.fire();
            }
        }));
        panel.add(note("Offset along chain (frames)")).colspan(2).width(innerW).padBottom(4).row();
        row(new UiButton(theme, "− 1", UiButton.Style.NORMAL, offset(-1)), new UiButton(theme, "+ 1", UiButton.Style.NORMAL, offset(1)));
        panel.add(note("Scale time")).colspan(2).width(innerW).padBottom(4).row();
        row(new UiButton(theme, "50%", UiButton.Style.NORMAL, scale(0.5)), new UiButton(theme, "200%", UiButton.Style.NORMAL, scale(2)));
        full(new UiButton(theme, "Delete keys", UiButton.Style.DANGER, new Runnable() {
            public void run() {
                int count = st.selectedKeys.size();
                st.editor.delete(sel());
                st.selectedKeys.clear();
                st.flash("Deleted " + count + " keys · Undo", new Runnable() {
                    public void run() {
                        st.doc.undo();
                    }
                });
            }
        }));
    }

    private UiButton pasteKeysButton() {
        UiButton b = new UiButton(theme, "Paste", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                int k = st.editor.paste(st.keyClipboard, st.frames.time(st.frame), null);
                st.flash("Pasted " + k + " keys at frame " + (st.frame + 1), null);
            }
        });
        b.enabled = st.keyClipboard != null;
        return b;
    }

    private UiButton ease(String label, final app.barkfart.model.Interp interp, final app.barkfart.model.Ease e) {
        return new UiButton(theme, label, UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.editor.setInterp(sel(), interp, e == null ? app.barkfart.model.Ease.LINEAR : e);
            }
        });
    }

    /** Stagger: each joint's selected keys shift by (its index along the hierarchy) x frames. */
    private Runnable offset(final int frames) {
        return new Runnable() {
            public void run() {
                java.util.LinkedHashMap<String, List<KeyRef>> byNode = new java.util.LinkedHashMap<String, List<KeyRef>>();
                for (Layer l : st.project().root().layers) {
                    if (!(l instanceof Layer.FigureLayer)) continue;
                    for (Node nd : ((Layer.FigureLayer) l).figure.nodes()) {
                        for (KeyRef k : st.selectedKeys) {
                            if (k.property == nd.rotation || k.property == nd.length || k.property == nd.thickness
                                    || k.property == nd.color || k.property == nd.shown) {
                                List<KeyRef> list = byNode.get(l.id + "/" + nd.id);
                                if (list == null) byNode.put(l.id + "/" + nd.id, list = new ArrayList<KeyRef>());
                                list.add(k);
                            }
                        }
                    }
                }
                st.editor.stagger(new ArrayList<List<KeyRef>>(byNode.values()), frames);
                st.selectedKeys.clear();
                st.flash("Offset " + byNode.size() + " joints by " + frames + " frame steps", null);
            }
        };
    }

    private Runnable scale(final double factor) {
        return new Runnable() {
            public void run() {
                double pivot = Double.MAX_VALUE;
                for (KeyRef k : st.selectedKeys) pivot = Math.min(pivot, k.time);
                st.editor.scaleTime(sel(), pivot, factor);
                st.selectedKeys.clear();
                st.fire();
            }
        };
    }

    private static int jointNumber(Layer.FigureLayer l, Node n) {
        return l.figure.indexOf(n.id);
    }

    private Runnable fps(final int d) {
        return new Runnable() {
            public void run() {
                float f = Math.max(1, Math.min(60, Math.round(st.editor.fps()) + d));
                st.doc.apply(StructureEdits.setFps(st.project(), f));
            }
        };
    }

    private Runnable uiSize(final float d) {
        return new Runnable() {
            public void run() {
                st.uiUserScale += d;
                // Rebuild after this event finishes (the button belongs to the UI being replaced).
                com.badlogic.gdx.Gdx.app.postRunnable(st.onUiScaleChanged);
            }
        };
    }

    private Runnable cycle(final int d) {
        return new Runnable() {
            public void run() {
                st.cycleFigure(d);
            }
        };
    }

    private Runnable stretch(final boolean on) {
        return new Runnable() {
            public void run() {
                st.tools.stretch = on;
                st.fire();
            }
        };
    }

    private void addFigure() {
        Layer.FigureLayer f = Demo.stickman(st.project(), "Figure " + (st.figures().size() + 1));
        f.transform.x.setValue(st.figures().size() * 40f);
        st.doc.apply(StructureEdits.addLayer(st.project().root(), f, st.project().root().layers.size()));
        st.select(f, null);
    }

    private void deleteFigure() {
        final Layer.FigureLayer l = st.selectedLayer;
        st.doc.apply(StructureEdits.removeLayer(st.project().root(), l));
        st.select(null, null);
        st.flash("Deleted " + l.name + " · Undo", new Runnable() {
            public void run() {
                st.doc.undo();
                st.select(l, null);
            }
        });
    }

    private void copyPose() {
        Layer.FigureLayer l = st.selectedLayer;
        double t = st.time();
        PropertyResolver r = new PropertyResolver(st.project(), new ArrayList<Diagnostic>());
        copiedPose = new HashMap<String, float[]>();
        for (Node n : l.figure.nodes()) copiedPose.put(n.id, new float[] {r.f(n.rotation, t), r.f(n.length, t)});
        st.flash("Copied pose of " + l.name, null);
    }

    private UiButton pasteButton() {
        UiButton b = new UiButton(theme, "Paste pose", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                Map<String, Float> rot = new HashMap<String, Float>(), len = new HashMap<String, Float>();
                for (Node n : st.selectedLayer.figure.nodes()) {
                    float[] v = copiedPose.get(n.id);
                    if (v == null) continue;
                    rot.put(n.id, v[0]);
                    len.put(n.id, v[1]);
                }
                st.editor.pose(st.selectedLayer, rot, len, st.frames.time(st.frame));
            }
        });
        b.enabled = copiedPose != null;
        return b;
    }

    /** Drag sideways to change the joint angle (1 degree per 4 pt), with - / + buttons. */
    static final class AngleScrub extends WidgetGroup {
        private final Theme theme;
        private final EditorState st;
        private final Node node;
        private final UiButton minus, plus;
        private final GlyphLayout gl = new GlyphLayout();
        private float lastX;

        AngleScrub(Theme theme, final EditorState st, final Node node) {
            this.theme = theme;
            this.st = st;
            this.node = node;
            minus = new UiButton(theme, "−", UiButton.Style.NORMAL, step(-1));
            plus = new UiButton(theme, "+", UiButton.Style.NORMAL, step(1));
            addActor(minus);
            addActor(plus);
            addListener(new InputListener() {
                @Override
                public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                    if (x < Theme.CONTROL_H || x > getWidth() - Theme.CONTROL_H) return false;
                    lastX = x;
                    st.doc.begin("Joint angle");
                    return true;
                }

                @Override
                public void touchDragged(InputEvent e, float x, float y, int pointer) {
                    float d = (x - lastX) / 4f;
                    if (Math.abs(d) < 1) return;
                    int whole = (int) d;
                    lastX += whole * 4;
                    st.editor.change(node.rotation, angle() + whole, st.frames.time(st.frame));
                }

                @Override
                public void touchUp(InputEvent e, float x, float y, int pointer, int button) {
                    st.doc.commit();
                }
            });
        }

        private float angle() {
            return (Float) node.rotation.keyValue(st.frames.time(st.frame));
        }

        private Runnable step(final int d) {
            return new Runnable() {
                public void run() {
                    st.editor.change(node.rotation, angle() + d, st.frames.time(st.frame));
                }
            };
        }

        @Override
        public void layout() {
            minus.setBounds(0, 0, Theme.CONTROL_H, getHeight());
            plus.setBounds(getWidth() - Theme.CONTROL_H, 0, Theme.CONTROL_H, getHeight());
        }

        @Override
        public void draw(Batch batch, float parentAlpha) {
            super.draw(batch, parentAlpha);
            String s = String.format("%.0f°", angle());
            theme.body.setColor(Theme.TEXT);
            gl.setText(theme.body, s);
            theme.body.draw(batch, gl, getX() + (getWidth() - gl.width) / 2, getY() + getHeight() / 2 + gl.height / 2);
        }
    }

}
