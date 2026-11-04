package org.opnlauncher.horizonfabricprobe;

import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.minecraft.client.Minecraft;
import net.minecraft.client.KeyMapping;
import net.minecraft.world.phys.Vec3;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.resources.Identifier;
import net.minecraft.network.chat.Component;

/** Test-only Fabric mod that exercises the mapped Horizon hooks without modifying its JAR. */
public final class FabricBridgeProbeMod implements ClientModInitializer {
    private static final AtomicInteger TICKS = new AtomicInteger();
    private static final AtomicInteger STARTS = new AtomicInteger();
    private static final AtomicBoolean SNAPSHOT_SHOWN = new AtomicBoolean();
    private static final KeyMapping.Category PROBE_CATEGORY = KeyMapping.Category.register(
            Identifier.fromNamespaceAndPath("opnlauncher", "probe"));
    private static final KeyMapping PROBE_KEY = KeyMappingHelper.registerKeyMapping(
            new KeyMapping("key.opnlauncher.probe.ping", InputConstants.Type.KEYBOARD,
                    InputConstants.KEY_P, PROBE_CATEGORY));

    @Override
    public void onInitializeClient() {
        if (!"opnlauncher:probe".equals(PROBE_CATEGORY.id().toString())
                || !"opnlauncher".equals(PROBE_CATEGORY.id().getNamespace())
                || !"probe".equals(PROBE_CATEGORY.id().getPath())) {
            throw new IllegalStateException("the bridge did not preserve the Fabric key category identifier");
        }
        try {
            KeyMapping.Category.register(Identifier.fromNamespaceAndPath("opnlauncher", "probe"));
            throw new IllegalStateException("the bridge accepted a duplicate Fabric key category identifier");
        } catch (IllegalArgumentException expected) {
            // Match Minecraft's duplicate category rejection.
        }
        if (KeyMappingHelper.getBoundKeyOf(PROBE_KEY).getValue() != InputConstants.KEY_P
                || !"key.keyboard.p".equals(KeyMappingHelper.getBoundKeyOf(PROBE_KEY).getName())) {
            throw new IllegalStateException("the bridge did not retain the registered default P key");
        }
        ClientTickEvents.START_CLIENT_TICK.register(client -> {
            if (client == null || client != Minecraft.getInstance()) {
                throw new IllegalStateException("the bridge did not provide its mapped Minecraft facade at tick start");
            }
            int start = STARTS.incrementAndGet();
            if (start == 1 && client.player == null) {
                client.gui.hud.setOverlayMessage(Component.literal("Horizon Fabric START_CLIENT_TICK"), false);
            } else if (start == 2 && client.player != null && client.level != null) {
                client.gui.hud.setOverlayMessage(
                        Component.literal("Fabric START sees " + client.player.getName().getString()
                                + " at " + client.level.getDayTime()
                                + " game " + client.level.getGameTime()
                                + " build " + client.level.getMinBuildHeight() + ".."
                                + client.level.getMaxBuildHeight()
                                + " rain " + client.level.isRaining() + "/"
                                + client.level.getRainLevel(1.0f)
                                + " thunder " + client.level.isThundering() + "/"
                                + client.level.getThunderLevel(1.0f)), false);
            } else if (PROBE_KEY.isDown()) {
                client.gui.hud.setOverlayMessage(Component.literal("Fabric key held P"), false);
            }
        });
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            if (client == null || client != Minecraft.getInstance()) {
                throw new IllegalStateException("the bridge did not provide its mapped Minecraft facade");
            }
            if (client.player != null) {
                KeyMapping sprintKey = client.options.keySprint;
                KeyMapping.set(sprintKey.getDefaultKey(), true);
                sprintKey.setDown(true);
            }
            int tick = TICKS.incrementAndGet();
            if (client.player != null && client.level != null && SNAPSHOT_SHOWN.compareAndSet(false, true)) {
                var velocity = client.player.getDeltaMovement();
                var blockPosition = client.player.blockPosition();
                var blockState = client.level.getBlockState(blockPosition);
                String player = client.player.getName().getString()
                        + " xyz " + client.player.getX() + "," + client.player.getY() + "," + client.player.getZ()
                        + " block " + blockPosition.getX() + "," + blockPosition.getY() + "," + blockPosition.getZ()
                        + " state " + blockState.getBlock().getDescriptionId()
                        + " air " + blockState.isAir() + " solid " + blockState.isSolid()
                        + " v " + velocity.x + "," + velocity.y + "," + velocity.z
                        + " yaw " + client.player.getYRot() + " pitch " + client.player.getXRot()
                        + " hp " + client.player.getHealth()
                        + " sprint " + client.player.isSprinting() + " ground " + client.player.isOnGround()
                        + " day " + client.level.getDayTime() + " game " + client.level.getGameTime()
                        + " height " + client.level.getMinBuildHeight() + ".."
                        + client.level.getMaxBuildHeight()
                        + " rain " + client.level.isRaining() + "/" + client.level.getRainLevel(1.0f)
                        + " thunder " + client.level.isThundering() + "/" + client.level.getThunderLevel(1.0f);
                client.gui.hud.setOverlayMessage(Component.literal("Fabric player " + player), false);
            } else if (client.player == null && tick == 1) {
                client.gui.hud.setOverlayMessage(Component.literal("Horizon Fabric END_CLIENT_TICK"), false);
            }
            if (client.player != null && PROBE_KEY.consumeClick()) {
                boolean sprinting = client.options.keyUp.isDown() && !client.options.keyShift.isDown();
                boolean forwardPressed = client.options.keyUp.consumeClick();
                KeyMapping.set(client.options.keySprint.getDefaultKey(), sprinting);
                client.options.keySprint.setDown(sprinting);
                Vec3 currentVelocity = client.player.getDeltaMovement();
                client.player.setDeltaMovement(currentVelocity.add(
                        -currentVelocity.x, 0.5 - currentVelocity.y, -currentVelocity.z));
                client.player.setYRot(client.player.getYRot() + 90.0f);
                client.player.setXRot(15.0f);
                client.player.sendSystemMessage(
                        Component.literal("Fabric system chat message mapped into Horizon"));
                client.gui.hud.setOverlayMessage(
                        Component.literal("Fabric forward=" + client.options.keyUp.isDown()
                                + " sneak=" + client.options.keyShift.isDown()
                                + " forwardPressed=" + forwardPressed
                                + "; sprint " + (sprinting ? "enabled" : "disabled")
                                + "; look direction and vertical velocity queued"), false);
            }
        });
    }

    public static int tickCount() {
        return TICKS.get();
    }

    public static int startCount() {
        return STARTS.get();
    }

    public static int registeredKeyCode() {
        return KeyMappingHelper.getBoundKeyOf(PROBE_KEY).getValue();
    }

}
