package net.minecraft.world.entity;

import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.Component;
import net.minecraft.world.phys.Vec3;
import org.opnlauncher.horizonfabric.HorizonFabricBridgeMain;

/** Snapshot facade with explicitly mapped local-player sprint, velocity, and look actions. */
public class Entity {
    private String name = "";
    private double x;
    private double y;
    private double z;
    private Vec3 deltaMovement = new Vec3(0, 0, 0);
    private float yRot;
    private float xRot;
    private boolean sprinting;
    private boolean onGround;

    protected void updateSnapshot(String name, double x, double y, double z,
                                  double velocityX, double velocityY, double velocityZ,
                                  float yRot, float xRot, boolean sprinting, boolean onGround) {
        this.name = name;
        this.x = x;
        this.y = y;
        this.z = z;
        this.deltaMovement = new Vec3(velocityX, velocityY, velocityZ);
        this.yRot = yRot;
        this.xRot = xRot;
        this.sprinting = sprinting;
        this.onGround = onGround;
    }

    public Component getName() { return Component.literal(name); }
    public double getX() { return x; }
    public double getY() { return y; }
    public double getZ() { return z; }
    /** Returns the block containing this entity's latest Horizon position snapshot. */
    public BlockPos blockPosition() {
        return new BlockPos((int) Math.floor(x), (int) Math.floor(y), (int) Math.floor(z));
    }
    public Vec3 getDeltaMovement() { return deltaMovement; }
    public float getYRot() { return yRot; }
    public float getXRot() { return xRot; }
    public boolean isSprinting() { return sprinting; }
    public boolean isOnGround() { return onGround; }

    /** Queues a bounded local-player yaw update for Horizon's Rust game thread. */
    public void setYRot(float yRot) {
        if (HorizonFabricBridgeMain.queuePlayerRotation(yRot, xRot)) {
            this.yRot = yRot;
        }
    }

    /** Queues a local-player pitch update; Horizon accepts only -90 through 90 degrees. */
    public void setXRot(float xRot) {
        if (HorizonFabricBridgeMain.queuePlayerRotation(yRot, xRot)) {
            this.xRot = xRot;
        }
    }

    /** Queues a local sprint-state change for Horizon's Rust game thread. */
    public void setSprinting(boolean sprinting) {
        this.sprinting = sprinting;
        HorizonFabricBridgeMain.queueSprinting(sprinting);
    }

    /** Queues a bounded velocity update for Horizon's native game thread. */
    public void setDeltaMovement(Vec3 movement) {
        if (HorizonFabricBridgeMain.queuePlayerVelocity(movement.x, movement.y, movement.z)) {
            this.deltaMovement = movement;
        }
    }

    /** Queues plain system chat text for Horizon's native chat UI. */
    public void sendSystemMessage(Component message) {
        if (message == null) throw new IllegalArgumentException("System message cannot be null");
        HorizonFabricBridgeMain.queueSystemMessage(message.getString());
    }
}
