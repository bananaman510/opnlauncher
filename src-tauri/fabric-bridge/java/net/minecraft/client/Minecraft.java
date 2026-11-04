package net.minecraft.client;

import net.minecraft.client.gui.Gui;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;

/**
 * Minimal 26.3 client facade for the Horizon sidecar. This is not a Minecraft
 * client instance; its player is a snapshot supplied by Horizon with one
 * explicitly mapped sprint and velocity actions.
 */
public final class Minecraft {
    private static final Minecraft INSTANCE = new Minecraft();

    public final Gui gui = new Gui();
    public final Options options = new Options();
    public ClientLevel level;
    public LocalPlayer player;

    private Minecraft() {}

    public static Minecraft getInstance() {
        return INSTANCE;
    }

    /** Called by the bridge before each callback; mod calls are rejected by the allowlist. */
    public void updateSnapshot(String name, double x, double y, double z,
                               double velocityX, double velocityY, double velocityZ,
                               float yRot, float xRot, float health,
                               boolean sprinting, boolean onGround, Long dayTime,
                               long gameTime, int minBuildHeight, int maxBuildHeight,
                               boolean raining, boolean thundering,
                               float rainLevelPrevious, float rainLevelCurrent,
                               float thunderLevelPrevious, float thunderLevelCurrent,
                               String nearbyBlocks) {
        if (dayTime == null) {
            level = null;
        } else {
            if (level == null) level = new ClientLevel();
            level.updateSnapshot(dayTime, gameTime, minBuildHeight, maxBuildHeight,
                    raining, thundering, rainLevelPrevious, rainLevelCurrent,
                    thunderLevelPrevious, thunderLevelCurrent, nearbyBlocks, x, y, z);
        }
        if (name == null) {
            player = null;
            return;
        }
        if (player == null) player = new LocalPlayer();
        player.updateSnapshot(name, x, y, z, velocityX, velocityY, velocityZ,
                yRot, xRot, health, sprinting, onGround);
    }
}
