package com.mojang.blaze3d.platform;

/** Keyboard-only Minecraft 26.3 InputConstants facade using SDL/HID key values. */
public final class InputConstants {
    public static final int KEY_0 = 39;
    public static final int KEY_1 = 30;
    public static final int KEY_2 = 31;
    public static final int KEY_3 = 32;
    public static final int KEY_4 = 33;
    public static final int KEY_5 = 34;
    public static final int KEY_6 = 35;
    public static final int KEY_7 = 36;
    public static final int KEY_8 = 37;
    public static final int KEY_9 = 38;
    public static final int KEY_A = 4;
    public static final int KEY_B = 5;
    public static final int KEY_C = 6;
    public static final int KEY_D = 7;
    public static final int KEY_E = 8;
    public static final int KEY_F = 9;
    public static final int KEY_G = 10;
    public static final int KEY_H = 11;
    public static final int KEY_I = 12;
    public static final int KEY_J = 13;
    public static final int KEY_K = 14;
    public static final int KEY_L = 15;
    public static final int KEY_M = 16;
    public static final int KEY_N = 17;
    public static final int KEY_O = 18;
    public static final int KEY_P = 19;
    public static final int KEY_Q = 20;
    public static final int KEY_R = 21;
    public static final int KEY_S = 22;
    public static final int KEY_T = 23;
    public static final int KEY_U = 24;
    public static final int KEY_V = 25;
    public static final int KEY_W = 26;
    public static final int KEY_X = 27;
    public static final int KEY_Y = 28;
    public static final int KEY_Z = 29;
    public static final int KEY_ADD = 87;
    public static final int KEY_APOSTROPHE = 52;
    public static final int KEY_BACKSLASH = 49;
    public static final int KEY_BACKSPACE = 42;
    public static final int KEY_CAPSLOCK = 57;
    public static final int KEY_COMMA = 54;
    public static final int KEY_DELETE = 76;
    public static final int KEY_DOWN = 81;
    public static final int KEY_END = 77;
    public static final int KEY_EQUALS = 46;
    public static final int KEY_ESCAPE = 41;
    public static final int KEY_F1 = 58;
    public static final int KEY_F2 = 59;
    public static final int KEY_F3 = 60;
    public static final int KEY_F4 = 61;
    public static final int KEY_F5 = 62;
    public static final int KEY_F6 = 63;
    public static final int KEY_F7 = 64;
    public static final int KEY_F8 = 65;
    public static final int KEY_F9 = 66;
    public static final int KEY_F10 = 67;
    public static final int KEY_F11 = 68;
    public static final int KEY_F12 = 69;
    public static final int KEY_F13 = 104;
    public static final int KEY_F14 = 105;
    public static final int KEY_F15 = 106;
    public static final int KEY_F16 = 107;
    public static final int KEY_F17 = 108;
    public static final int KEY_F18 = 109;
    public static final int KEY_F19 = 110;
    public static final int KEY_F20 = 111;
    public static final int KEY_F21 = 112;
    public static final int KEY_F22 = 113;
    public static final int KEY_F23 = 114;
    public static final int KEY_F24 = 115;
    public static final int KEY_GRAVE = 53;
    public static final int KEY_HOME = 74;
    public static final int KEY_INSERT = 73;
    public static final int KEY_LALT = 226;
    public static final int KEY_LBRACKET = 47;
    public static final int KEY_LCONTROL = 224;
    public static final int KEY_LEFT = 80;
    public static final int KEY_LGUI = 227;
    public static final int KEY_LSHIFT = 225;
    public static final int KEY_MINUS = 45;
    public static final int KEY_MULTIPLY = 85;
    public static final int KEY_NUMLOCK = 83;
    public static final int KEY_NUMPAD0 = 98;
    public static final int KEY_NUMPAD1 = 89;
    public static final int KEY_NUMPAD2 = 90;
    public static final int KEY_NUMPAD3 = 91;
    public static final int KEY_NUMPAD4 = 92;
    public static final int KEY_NUMPAD5 = 93;
    public static final int KEY_NUMPAD6 = 94;
    public static final int KEY_NUMPAD7 = 95;
    public static final int KEY_NUMPAD8 = 96;
    public static final int KEY_NUMPAD9 = 97;
    public static final int KEY_NUMPADCOMMA = 220;
    public static final int KEY_NUMPADENTER = 88;
    public static final int KEY_NUMPADEQUALS = 103;
    public static final int KEY_PAGEDOWN = 78;
    public static final int KEY_PAGEUP = 75;
    public static final int KEY_PAUSE = 72;
    public static final int KEY_PERIOD = 55;
    public static final int KEY_PRINTSCREEN = 70;
    public static final int KEY_RALT = 230;
    public static final int KEY_RBRACKET = 48;
    public static final int KEY_RCONTROL = 228;
    public static final int KEY_RETURN = 40;
    public static final int KEY_RGUI = 231;
    public static final int KEY_RIGHT = 79;
    public static final int KEY_RSHIFT = 229;
    public static final int KEY_SCROLLLOCK = 71;
    public static final int KEY_SEMICOLON = 51;
    public static final int KEY_SLASH = 56;
    public static final int KEY_SPACE = 44;
    public static final int KEY_TAB = 43;
    public static final int KEY_UP = 82;

    private InputConstants() {}

    public enum Type {
        KEYBOARD,
        MOUSE;

        public Key create(int value) {
            if (this != KEYBOARD) {
                throw new UnsupportedOperationException("Only keyboard key mappings are supported by Horizon");
            }
            if (!isSupportedKeyboardKeyCode(value)) {
                throw new UnsupportedOperationException("The requested key is outside Horizon's mapped Minecraft 26.3 keyboard subset");
            }
            return new Key(value);
        }
    }

    private static boolean isSupportedKeyboardKeyCode(int value) {
        return (value >= KEY_A && value <= KEY_Z)
                || (value >= KEY_1 && value <= KEY_0)
                || (value >= KEY_F1 && value <= KEY_F12)
                || (value >= KEY_F13 && value <= KEY_F24)
                || (value >= KEY_NUMPAD1 && value <= KEY_NUMPAD9)
                || (value >= KEY_LCONTROL && value <= KEY_RGUI)
                || (value >= 144 && value <= 148)
                || (value >= 121 && value <= 126)
                || switch (value) {
                    case KEY_ADD, KEY_APOSTROPHE, KEY_BACKSLASH, KEY_BACKSPACE, KEY_CAPSLOCK, KEY_COMMA,
                            KEY_DELETE, KEY_DOWN, KEY_END, KEY_EQUALS, KEY_ESCAPE, KEY_GRAVE, KEY_HOME,
                            KEY_INSERT, KEY_LALT, KEY_LBRACKET, KEY_LEFT, KEY_LSHIFT, KEY_MINUS,
                            KEY_MULTIPLY, KEY_NUMLOCK, KEY_NUMPAD0, KEY_NUMPADCOMMA, KEY_NUMPADENTER,
                            KEY_NUMPADEQUALS, KEY_PAGEDOWN, KEY_PAGEUP, KEY_PAUSE, KEY_PERIOD,
                            KEY_PRINTSCREEN, KEY_RALT, KEY_RBRACKET, KEY_RETURN, KEY_RIGHT, KEY_SCROLLLOCK,
                            KEY_SEMICOLON, KEY_SLASH, KEY_SPACE, KEY_TAB, KEY_UP,
                            100, 117, 119 -> true;
                    default -> false;
                };
    }

    public static final class Key {
        private final int value;

        private Key(int value) {
            this.value = value;
        }

        public int getValue() {
            return value;
        }

        public Type getType() {
            return Type.KEYBOARD;
        }

        public String getName() {
            if (value >= KEY_A && value <= KEY_Z) return "key.keyboard." + (char) ('a' + value - KEY_A);
            if (value >= KEY_1 && value <= KEY_9) return "key.keyboard." + (char) ('1' + value - KEY_1);
            if (value == KEY_0) return "key.keyboard.0";
            if (value >= KEY_F1 && value <= KEY_F12) return "key.keyboard.f" + (value - KEY_F1 + 1);
            if (value >= KEY_F13 && value <= KEY_F24) return "key.keyboard.f" + (value - KEY_F13 + 13);
            if (value >= KEY_NUMPAD1 && value <= KEY_NUMPAD9) return "key.keyboard.keypad." + (value - KEY_NUMPAD1 + 1);
            if (value == KEY_NUMPAD0) return "key.keyboard.keypad.0";
            if (value == 135) return "key.keyboard.international1";
            if (value == 137) return "key.keyboard.international3";
            if (value >= 144 && value <= 148) return "key.keyboard.lang" + (value - 144 + 1);
            if (value >= 121 && value <= 126) {
                return switch (value) {
                    case 121 -> "key.keyboard.again";
                    case 122 -> "key.keyboard.undo";
                    case 123 -> "key.keyboard.cut";
                    case 124 -> "key.keyboard.copy";
                    case 125 -> "key.keyboard.paste";
                    case 126 -> "key.keyboard.find";
                    default -> "key.keyboard.unknown";
                };
            }
            return switch (value) {
                case KEY_ADD -> "key.keyboard.keypad.add";
                case KEY_APOSTROPHE -> "key.keyboard.apostrophe";
                case KEY_BACKSLASH -> "key.keyboard.backslash";
                case KEY_BACKSPACE -> "key.keyboard.backspace";
                case KEY_CAPSLOCK -> "key.keyboard.caps.lock";
                case KEY_COMMA -> "key.keyboard.comma";
                case KEY_DELETE -> "key.keyboard.delete";
                case KEY_DOWN -> "key.keyboard.down";
                case KEY_END -> "key.keyboard.end";
                case KEY_EQUALS -> "key.keyboard.equal";
                case KEY_ESCAPE -> "key.keyboard.escape";
                case KEY_GRAVE -> "key.keyboard.grave.accent";
                case 100 -> "key.keyboard.world.1";
                case 117 -> "key.keyboard.help";
                case KEY_HOME -> "key.keyboard.home";
                case KEY_INSERT -> "key.keyboard.insert";
                case KEY_LALT -> "key.keyboard.left.alt";
                case KEY_LBRACKET -> "key.keyboard.left.bracket";
                case KEY_LCONTROL -> "key.keyboard.left.control";
                case KEY_LEFT -> "key.keyboard.left";
                case KEY_LGUI -> "key.keyboard.left.win";
                case KEY_LSHIFT -> "key.keyboard.left.shift";
                case KEY_MINUS -> "key.keyboard.minus";
                case KEY_MULTIPLY -> "key.keyboard.keypad.multiply";
                case KEY_NUMLOCK -> "key.keyboard.num.lock";
                case KEY_NUMPADCOMMA -> "key.keyboard.keypad.comma";
                case KEY_NUMPADENTER -> "key.keyboard.keypad.enter";
                case KEY_NUMPADEQUALS -> "key.keyboard.keypad.equal";
                case KEY_PAGEDOWN -> "key.keyboard.page.down";
                case KEY_PAGEUP -> "key.keyboard.page.up";
                case KEY_PAUSE -> "key.keyboard.pause";
                case KEY_PERIOD -> "key.keyboard.period";
                case KEY_PRINTSCREEN -> "key.keyboard.print.screen";
                case KEY_RALT -> "key.keyboard.right.alt";
                case KEY_RBRACKET -> "key.keyboard.right.bracket";
                case KEY_RCONTROL -> "key.keyboard.right.control";
                case KEY_RETURN -> "key.keyboard.enter";
                case KEY_RGUI -> "key.keyboard.right.win";
                case KEY_RIGHT -> "key.keyboard.right";
                case KEY_RSHIFT -> "key.keyboard.right.shift";
                case KEY_SCROLLLOCK -> "key.keyboard.scroll.lock";
                case KEY_SEMICOLON -> "key.keyboard.semicolon";
                case KEY_SLASH -> "key.keyboard.slash";
                case KEY_SPACE -> "key.keyboard.space";
                case KEY_TAB -> "key.keyboard.tab";
                case KEY_UP -> "key.keyboard.up";
                default -> "key.keyboard.unknown";
            };
        }

        @Override
        public boolean equals(Object other) {
            return other instanceof Key key && key.value == value;
        }

        @Override
        public int hashCode() {
            return Integer.hashCode(value);
        }
    }
}
