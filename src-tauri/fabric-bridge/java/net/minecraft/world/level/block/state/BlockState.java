package net.minecraft.world.level.block.state;

import net.minecraft.world.level.block.Block;

/** Minimal immutable block facts from Horizon; properties, fluids, and behaviors are unmapped. */
public final class BlockState {
    private final String blockId;
    private final boolean air;
    private final boolean fullCollision;
    private final boolean solidRender;
    private final Block block;

    public BlockState(String blockId, boolean air, boolean fullCollision, boolean solidRender) {
        this.blockId = blockId;
        this.air = air;
        this.fullCollision = fullCollision;
        this.solidRender = solidRender;
        this.block = new Block(blockId);
    }

    public Block getBlock() { return block; }
    public boolean isAir() { return air; }
    public boolean isSolid() { return fullCollision; }
    public boolean isSolidRender() { return solidRender; }

    @Override
    public String toString() { return blockId; }
}
