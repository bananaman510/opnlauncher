package net.minecraft.world.phys;

/** Immutable vector facade for values copied from the Horizon player snapshot. */
public final class Vec3 {
    public final double x;
    public final double y;
    public final double z;

    public Vec3(double x, double y, double z) {
        this.x = x;
        this.y = y;
        this.z = z;
    }

    public Vec3 add(double x, double y, double z) {
        return new Vec3(this.x + x, this.y + y, this.z + z);
    }

    public double lengthSqr() { return x * x + y * y + z * z; }
    public double length() { return Math.sqrt(lengthSqr()); }
    public double horizontalDistanceSqr() { return x * x + z * z; }
    public double horizontalDistance() { return Math.sqrt(horizontalDistanceSqr()); }
}
