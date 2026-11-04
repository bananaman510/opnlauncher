package net.minecraft.client;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.fabricmc.fabric.mixin.client.keymapping.KeyMappingAccessor;
import net.minecraft.resources.Identifier;
import org.opnlauncher.horizonfabric.HorizonFabricBridgeMain;

/**
 * Read-only input facade backed by keyboard snapshots from the native Horizon game.
 * It intentionally has no options-screen rebinding or mouse/controller support.
 */
public final class KeyMapping implements KeyMappingAccessor {
    private static final int MAX_REGISTERED_MAPPINGS = 128;
    private static final int MAX_REGISTERED_CATEGORIES = 128;
    private static final int INPUT_SPRINT = 1 << 6;
    private static final List<KeyMapping> REGISTERED = new ArrayList<>();
    private static final Set<Identifier> REGISTERED_CATEGORY_IDS = new HashSet<>();

    private final String name;
    private final InputConstants.Key key;
    private final Category category;
    private final int nativeInputMask;
    private boolean down;
    private int clickCount;

    public KeyMapping(String name, InputConstants.Type type, int keyCode, Category category) {
        this(name, type, keyCode, category, 0);
    }

    private KeyMapping(String name, InputConstants.Type type, int keyCode, Category category, int nativeInputMask) {
        if (name == null || name.isBlank() || name.length() > 128 || name.codePoints().anyMatch(Character::isISOControl)) {
            throw new IllegalArgumentException("key mapping name must be short, non-empty, and contain no control characters");
        }
        if (type == null || category == null) throw new IllegalArgumentException("key mapping type and category are required");
        if (nativeInputMask < 0 || nativeInputMask > 0x7f || Integer.bitCount(nativeInputMask) > 1) {
            throw new IllegalArgumentException("native input mask is outside the mapped Horizon controls");
        }
        this.name = name;
        this.key = type.create(keyCode);
        this.category = category;
        this.nativeInputMask = nativeInputMask;
    }

    public synchronized String getName() {
        return name;
    }

    public synchronized InputConstants.Key getKey() {
        return key;
    }

    public synchronized InputConstants.Key getDefaultKey() {
        return key;
    }

    public synchronized Category getCategory() {
        return category;
    }

    @Override
    public synchronized InputConstants.Key fabric_getBoundKey() {
        return key;
    }

    public synchronized boolean isDown() {
        return down;
    }

    /**
     * Maps Minecraft's programmatic key state for the native sprint binding
     * back into Horizon's queued Rust sprint action. Other bindings only update
     * this sidecar facade and do not synthesize Horizon input.
     */
    public synchronized void setDown(boolean down) {
        if (this.down == down) return;
        this.down = down;
        if (nativeInputMask == INPUT_SPRINT) {
            HorizonFabricBridgeMain.queueSprinting(down);
        }
    }

    /** Maps Minecraft's global keyboard state update onto registered bindings. */
    public static void set(InputConstants.Key key, boolean down) {
        if (key == null) throw new IllegalArgumentException("key cannot be null");
        List<KeyMapping> matching;
        synchronized (KeyMapping.class) {
            matching = List.copyOf(REGISTERED);
        }
        for (KeyMapping mapping : matching) {
            if (mapping.key.equals(key)) mapping.setDown(down);
        }
    }

    public synchronized boolean consumeClick() {
        if (clickCount == 0) return false;
        clickCount--;
        return true;
    }

    public static synchronized KeyMapping register(KeyMapping mapping) {
        if (mapping == null) throw new IllegalArgumentException("key mapping cannot be null");
        if (REGISTERED.size() >= MAX_REGISTERED_MAPPINGS) {
            throw new IllegalStateException("Horizon allows at most " + MAX_REGISTERED_MAPPINGS + " Fabric key mappings");
        }
        if (REGISTERED.stream().anyMatch(existing -> existing.name.equals(mapping.name))) {
            throw new IllegalArgumentException("A key mapping with ID '" + mapping.name + "' is already registered");
        }
        REGISTERED.add(mapping);
        return mapping;
    }

    static KeyMapping nativeBinding(String name, int keyCode, Category category, int nativeInputMask) {
        return register(new KeyMapping(name, InputConstants.Type.KEYBOARD, keyCode, category, nativeInputMask));
    }

    /** Called once before each Fabric tick callback with a bounded native input snapshot. */
    public static synchronized void updateBridgeInput(List<Integer> downKeys, List<Integer> pressedKeys, int nativeInputFlags) {
        if (nativeInputFlags < 0 || (nativeInputFlags & ~0x7f) != 0) {
            throw new IllegalArgumentException("native input flags are outside the mapped Horizon controls");
        }
        for (KeyMapping mapping : REGISTERED) {
            boolean wasDown = mapping.down;
            int code = mapping.key.getValue();
            boolean isDown;
            if (mapping.nativeInputMask != 0) {
                isDown = (nativeInputFlags & mapping.nativeInputMask) != 0;
                if (isDown && !wasDown && mapping.clickCount < 64) mapping.clickCount++;
            } else {
                isDown = downKeys.contains(code);
            }
            mapping.down = isDown;
            if (mapping.nativeInputMask != 0) continue;
            for (int pressed : pressedKeys) {
                if (pressed == code && mapping.clickCount < 64) mapping.clickCount++;
            }
        }
    }

    public record Category(Identifier id) {
        public static final Category MOVEMENT = builtin("movement");
        public static final Category MISC = builtin("misc");
        public static final Category MULTIPLAYER = builtin("multiplayer");
        public static final Category GAMEPLAY = builtin("gameplay");
        public static final Category INVENTORY = builtin("inventory");
        public static final Category CREATIVE = builtin("creative");
        public static final Category SPECTATOR = builtin("spectator");
        public static final Category DEBUG = builtin("debug");
        public static final Category SYSTEM = builtin("system");

        public Category {
            if (id == null) throw new IllegalArgumentException("key category identifier cannot be null");
        }

        private static Category builtin(String path) {
            Identifier id = Identifier.fromNamespaceAndPath("minecraft", path);
            REGISTERED_CATEGORY_IDS.add(id);
            return new Category(id);
        }

        public static synchronized Category register(Identifier id) {
            if (id == null) throw new IllegalArgumentException("key category identifier cannot be null");
            if (REGISTERED_CATEGORY_IDS.size() >= MAX_REGISTERED_CATEGORIES) {
                throw new IllegalStateException("Horizon allows at most " + MAX_REGISTERED_CATEGORIES
                        + " Fabric key categories");
            }
            if (!REGISTERED_CATEGORY_IDS.add(id)) {
                throw new IllegalArgumentException("A key category with ID '" + id + "' is already registered");
            }
            return new Category(id);
        }
    }
}
