package net.minecraft.world.level.block;

/** Minimal immutable block identity supplied by Horizon's nearby-world snapshot. */
public final class Block {
    private final String descriptionId;

    public Block(String descriptionId) {
        this.descriptionId = descriptionId;
    }

    public String getDescriptionId() { return descriptionId; }

    @Override
    public String toString() { return "Block{" + descriptionId + "}"; }
}
