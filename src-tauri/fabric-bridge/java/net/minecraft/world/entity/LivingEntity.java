package net.minecraft.world.entity;

/** Read-only health values for the mapped local-player snapshot. */
public class LivingEntity extends Entity {
    private float health;

    protected void updateSnapshot(String name, double x, double y, double z,
                                  double velocityX, double velocityY, double velocityZ,
                                  float yRot, float xRot, float health,
                                  boolean sprinting, boolean onGround) {
        super.updateSnapshot(name, x, y, z, velocityX, velocityY, velocityZ,
                yRot, xRot, sprinting, onGround);
        this.health = health;
    }

    public float getHealth() { return health; }
    public boolean isAlive() { return health > 0.0f; }
}
