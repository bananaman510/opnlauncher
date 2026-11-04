package net.fabricmc.fabric.api.client.keymapping.v1;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyMapping;

/** Mapped Fabric API 26.3 key registration helpers backed by Horizon keyboard input. */
public final class KeyMappingHelper {
    private KeyMappingHelper() {}

    public static KeyMapping registerKeyMapping(KeyMapping mapping) {
        return KeyMapping.register(mapping);
    }

    public static InputConstants.Key getBoundKeyOf(KeyMapping mapping) {
        if (mapping == null) throw new IllegalArgumentException("key mapping cannot be null");
        return mapping.fabric_getBoundKey();
    }
}
