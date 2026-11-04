package net.minecraft.client;

import com.mojang.blaze3d.platform.InputConstants;

/** Read-only vanilla movement bindings backed by Horizon's semantic input snapshot. */
public final class Options {
    private static final int INPUT_UP = 1 << 0;
    private static final int INPUT_DOWN = 1 << 1;
    private static final int INPUT_LEFT = 1 << 2;
    private static final int INPUT_RIGHT = 1 << 3;
    private static final int INPUT_JUMP = 1 << 4;
    private static final int INPUT_SHIFT = 1 << 5;
    private static final int INPUT_SPRINT = 1 << 6;

    Options() {}

    public final KeyMapping keyUp = KeyMapping.nativeBinding(
            "key.forward", InputConstants.KEY_W, KeyMapping.Category.MOVEMENT, INPUT_UP);
    public final KeyMapping keyDown = KeyMapping.nativeBinding(
            "key.back", InputConstants.KEY_S, KeyMapping.Category.MOVEMENT, INPUT_DOWN);
    public final KeyMapping keyLeft = KeyMapping.nativeBinding(
            "key.left", InputConstants.KEY_A, KeyMapping.Category.MOVEMENT, INPUT_LEFT);
    public final KeyMapping keyRight = KeyMapping.nativeBinding(
            "key.right", InputConstants.KEY_D, KeyMapping.Category.MOVEMENT, INPUT_RIGHT);
    public final KeyMapping keyJump = KeyMapping.nativeBinding(
            "key.jump", InputConstants.KEY_SPACE, KeyMapping.Category.MOVEMENT, INPUT_JUMP);
    public final KeyMapping keyShift = KeyMapping.nativeBinding(
            "key.sneak", InputConstants.KEY_LSHIFT, KeyMapping.Category.MOVEMENT, INPUT_SHIFT);
    public final KeyMapping keySprint = KeyMapping.nativeBinding(
            "key.sprint", InputConstants.KEY_LCONTROL, KeyMapping.Category.MOVEMENT, INPUT_SPRINT);
}
