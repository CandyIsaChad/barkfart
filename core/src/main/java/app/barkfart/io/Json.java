package app.barkfart.io;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal deterministic JSON. Values: Map (insertion-ordered), List, String,
 * Boolean, null, and Num (the number's exact source text, so floats and
 * doubles parse back bit-identically with Float/Double.parseX).
 */
public final class Json {
    private Json() {}

    /** A JSON number kept as text. */
    public static final class Num {
        public final String text;

        public Num(String text) {
            this.text = text;
        }

        public float f() {
            return Float.parseFloat(text);
        }

        public double d() {
            return Double.parseDouble(text);
        }

        public int i() {
            return Integer.parseInt(text);
        }
    }

    public static Num num(float v) {
        return new Num(Float.isNaN(v) || Float.isInfinite(v) ? "0" : Float.toString(v));
    }

    public static Num num(double v) {
        return new Num(Double.isNaN(v) || Double.isInfinite(v) ? "0" : Double.toString(v));
    }

    public static Num num(int v) {
        return new Num(Integer.toString(v));
    }

    // ---- writing ----

    public static String write(Object v) {
        StringBuilder sb = new StringBuilder();
        write(v, sb, 0);
        sb.append('\n');
        return sb.toString();
    }

    @SuppressWarnings("unchecked")
    private static void write(Object v, StringBuilder sb, int indent) {
        if (v == null) {
            sb.append("null");
        } else if (v instanceof Num) {
            sb.append(((Num) v).text);
        } else if (v instanceof Boolean) {
            sb.append(v.toString());
        } else if (v instanceof String) {
            str((String) v, sb);
        } else if (v instanceof Map) {
            Map<String, Object> m = (Map<String, Object>) v;
            if (m.isEmpty()) {
                sb.append("{}");
                return;
            }
            sb.append('{');
            boolean first = true;
            for (Map.Entry<String, Object> e : m.entrySet()) {
                if (!first) sb.append(',');
                first = false;
                nl(sb, indent + 1);
                str(e.getKey(), sb);
                sb.append(": ");
                write(e.getValue(), sb, indent + 1);
            }
            nl(sb, indent);
            sb.append('}');
        } else if (v instanceof List) {
            List<Object> l = (List<Object>) v;
            boolean flat = true;
            for (Object o : l) flat &= !(o instanceof Map) && !(o instanceof List && !((List<?>) o).isEmpty() && isDeep((List<?>) o));
            sb.append('[');
            for (int i = 0; i < l.size(); i++) {
                if (i > 0) sb.append(flat ? ", " : ",");
                if (!flat) nl(sb, indent + 1);
                write(l.get(i), sb, indent + 1);
            }
            if (!flat && !l.isEmpty()) nl(sb, indent);
            sb.append(']');
        } else {
            throw new IllegalArgumentException("not a JSON value: " + v.getClass());
        }
    }

    private static boolean isDeep(List<?> l) {
        for (Object o : l) if (o instanceof Map || o instanceof List) return true;
        return false;
    }

    private static void nl(StringBuilder sb, int indent) {
        sb.append('\n');
        for (int i = 0; i < indent; i++) sb.append("  ");
    }

    private static void str(String s, StringBuilder sb) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"': sb.append("\\\""); break;
                case '\\': sb.append("\\\\"); break;
                case '\n': sb.append("\\n"); break;
                case '\r': sb.append("\\r"); break;
                case '\t': sb.append("\\t"); break;
                default:
                    if (c < 0x20) sb.append(String.format("\\u%04x", (int) c));
                    else sb.append(c);
            }
        }
        sb.append('"');
    }

    // ---- parsing ----

    public static Object parse(String s) {
        Parser p = new Parser(s);
        p.ws();
        Object v = p.value(0);
        p.ws();
        if (p.i != s.length()) throw p.err("trailing data");
        return v;
    }

    private static final class Parser {
        private static final int MAX_DEPTH = 512;
        final String s;
        int i;

        Parser(String s) {
            this.s = s;
        }

        IllegalArgumentException err(String m) {
            return new IllegalArgumentException("JSON " + m + " at " + i);
        }

        void ws() {
            while (i < s.length() && Character.isWhitespace(s.charAt(i))) i++;
        }

        Object value(int depth) {
            if (depth > MAX_DEPTH) throw err("too deep");
            if (i >= s.length()) throw err("unexpected end");
            char c = s.charAt(i);
            if (c == '{') return obj(depth);
            if (c == '[') return arr(depth);
            if (c == '"') return string();
            if (s.startsWith("true", i)) {
                i += 4;
                return Boolean.TRUE;
            }
            if (s.startsWith("false", i)) {
                i += 5;
                return Boolean.FALSE;
            }
            if (s.startsWith("null", i)) {
                i += 4;
                return null;
            }
            return number();
        }

        Map<String, Object> obj(int depth) {
            LinkedHashMap<String, Object> m = new LinkedHashMap<String, Object>();
            i++;
            ws();
            if (peek() == '}') {
                i++;
                return m;
            }
            while (true) {
                ws();
                if (peek() != '"') throw err("expected key");
                String k = string();
                ws();
                if (peek() != ':') throw err("expected ':'");
                i++;
                ws();
                m.put(k, value(depth + 1));
                ws();
                char c = peek();
                i++;
                if (c == '}') return m;
                if (c != ',') throw err("expected ',' or '}'");
            }
        }

        List<Object> arr(int depth) {
            ArrayList<Object> l = new ArrayList<Object>();
            i++;
            ws();
            if (peek() == ']') {
                i++;
                return l;
            }
            while (true) {
                ws();
                l.add(value(depth + 1));
                ws();
                char c = peek();
                i++;
                if (c == ']') return l;
                if (c != ',') throw err("expected ',' or ']'");
            }
        }

        char peek() {
            if (i >= s.length()) throw err("unexpected end");
            return s.charAt(i);
        }

        String string() {
            StringBuilder sb = new StringBuilder();
            i++;
            while (true) {
                char c = peek();
                i++;
                if (c == '"') return sb.toString();
                if (c != '\\') {
                    sb.append(c);
                    continue;
                }
                char e = peek();
                i++;
                switch (e) {
                    case 'n': sb.append('\n'); break;
                    case 'r': sb.append('\r'); break;
                    case 't': sb.append('\t'); break;
                    case 'b': sb.append('\b'); break;
                    case 'f': sb.append('\f'); break;
                    case 'u':
                        if (i + 4 > s.length()) throw err("bad escape");
                        sb.append((char) Integer.parseInt(s.substring(i, i + 4), 16));
                        i += 4;
                        break;
                    default: sb.append(e);
                }
            }
        }

        Num number() {
            int st = i;
            while (i < s.length() && "+-0123456789.eE".indexOf(s.charAt(i)) >= 0) i++;
            if (st == i) throw err("unexpected character");
            String t = s.substring(st, i);
            try {
                Double.parseDouble(t);
            } catch (NumberFormatException ex) {
                throw err("bad number " + t);
            }
            return new Num(t);
        }
    }
}
