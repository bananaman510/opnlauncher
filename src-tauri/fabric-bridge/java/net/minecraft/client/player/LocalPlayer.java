package net.minecraft.client.player;

import net.minecraft.world.entity.player.Player;

/** Local-player snapshot facade with mapped sprint and velocity actions. */
public class LocalPlayer extends Player {
    public void updateSnapshot(String name, double x, double y, double z,
                               double velocityX, double velocityY, double velocityZ,
                               float yRot, float xRot, float health,
                               boolean sprinting, boolean onGround) {
        super.updateSnapshot(name, x, y, z, velocityX, velocityY, velocityZ,
                yRot, xRot, health, sprinting, onGround);
    }
}
