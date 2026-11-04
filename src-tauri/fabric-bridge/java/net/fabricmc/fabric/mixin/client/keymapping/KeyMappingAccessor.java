package net.fabricmc.fabric.mixin.client.keymapping;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyMapping;

/** Binary-compatible accessor signature used by Fabric API's KeyMappingHelper. */
public interface KeyMappingAccessor {
    InputConstants.Key fabric_getBoundKey();
}
