package org.opnlauncher.horizonfabric;

import java.io.BufferedReader;
import java.io.PrintWriter;
import java.io.StringReader;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import java.util.concurrent.atomic.AtomicInteger;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.loader.impl.FabricLoaderImpl;

public final class HorizonFabricBridgeTest {
    private static final AtomicInteger MAIN_ENTRYPOINTS = new AtomicInteger();
    private HorizonFabricBridgeTest() {}

    public static void main(String[] args) throws Exception {
        ClientTickEvents.START_CLIENT_TICK.register(client -> {});
        try {
            ClientTickEvents.START_LEVEL_TICK.register(level -> {});
            throw new AssertionError("unsupported lifecycle event was accepted");
        } catch (UnsupportedOperationException expected) {
            // The bridge must fail clearly when a mod asks for an unmapped event.
        }

        ClassLoader loader = HorizonFabricBridgeTest.class.getClassLoader();
        net.minecraft.world.phys.Vec3 vector = new net.minecraft.world.phys.Vec3(1.0, 2.0, 3.0);
        net.minecraft.world.phys.Vec3 offset = vector.add(-1.0, 0.5, 2.0);
        require(offset.x == 0.0 && offset.y == 2.5 && offset.z == 5.0,
                "the mapped Vec3.add operation did not return the translated vector");
        require(vector.x == 1.0 && vector.y == 2.0 && vector.z == 3.0,
                "the mapped Vec3.add operation mutated its source vector");
        require(HorizonFabricBridgeMain.invokeEntrypoints(
                loader, "main", "net.fabricmc.api.ModInitializer", "onInitialize"),
                "Fabric main initializer stage failed");
        require(HorizonFabricBridgeMain.invokeEntrypoints(
                loader, "client", "net.fabricmc.api.ClientModInitializer", "onInitializeClient"),
                "Fabric client initializer stage failed");
        StringWriter acknowledgements = new StringWriter();
        HorizonFabricBridgeMain.dispatchTicks(
                loader,
                new BufferedReader(new StringReader(
                                "start\t-\t-\t0\t-\t-\t-\n"
                                + "end\t-\t-\t0\t-\t-\t-\n"
                                + "start\t5465737465725f32\t1.25\t64.0\t-3.5\t0.3\t0.4\t0.0\t37.5\t-12.5\t18.5\ttrue\tfalse\t6000\t300\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t0\t-\t-\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n"
                                + "end\t5465737465725f32\t1.25\t64.0\t-3.5\t0.3\t0.4\t0.0\t37.5\t-12.5\t18.5\ttrue\tfalse\t6001\t301\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t0\t-\t-\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n"
                                + "start\t5465737465725f32\t1.25\t64.0\t-3.5\t0.3\t0.4\t0.0\t37.5\t-12.5\t18.5\ttrue\tfalse\t6002\t302\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t0\t19\t19\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n"
                                + "end\t5465737465725f32\t1.25\t64.0\t-3.5\t0.3\t0.4\t0.0\t37.5\t-12.5\t18.5\ttrue\tfalse\t6003\t303\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t1\t-\t-\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n"
                                + "start\t5465737465725f32\t1.25\t64.0\t-3.5\tNaN\t0.4\t0.0\t37.5\t-12.5\t18.5\ttrue\tfalse\t6004\t304\t-64\t320\ttrue\tfalse\t0.25\t0.5\t0.1\t0.2\t0\t-\t-\t1,64,-4,minecraft:stone,0,1,1;1,63,-4,minecraft:bedrock,0,1,1\n"
                                + "stop\n")),
                new PrintWriter(acknowledgements, true));
        String[] responses = acknowledgements.toString().split("\\R");
        require(responses.length == 7, "the bridge did not acknowledge native phases or reject an invalid snapshot");
        require(responses[0].equals("tick\t1\tA0:486f72697a6f6e204661627269632053544152545f434c49454e545f5449434b\t-\t-\t-\t-"),
                "START_CLIENT_TICK action was not mapped to the Rust response");
        require(responses[1].equals("tick\t2\tA0:486f72697a6f6e2046616272696320454e445f434c49454e545f5449434b\t-\t-\t-\t-"),
                "END_CLIENT_TICK action was not mapped to the Rust response");
        String environmentMessage = "Fabric START sees Tester_2 at 6000 game 300 build -64..320 rain true/0.5 thunder false/0.2";
        require(responses[2].equals("tick\t3\tA0:" + HexFormat.of().formatHex(environmentMessage.getBytes(StandardCharsets.UTF_8)) + "\t-\t-\t-\t-"),
                "START_CLIENT_TICK did not receive the Rust player and environmental snapshots");
        String playerMessage = "Fabric player Tester_2 xyz 1.25,64.0,-3.5 block 1,64,-4 state minecraft:stone air false solid true v 0.3,0.4,0.0 yaw 37.5 pitch -12.5 hp 18.5 sprint true ground false day 6001 game 301 height -64..320 rain true/0.5 thunder false/0.2";
        require(responses[3].equals("tick\t4\tA0:" + HexFormat.of().formatHex(playerMessage.getBytes(StandardCharsets.UTF_8)) + "\tS1\t-\t-\t-"),
                "END_CLIENT_TICK player and environmental snapshots were not mapped through the action-bar response: " + responses[3]);
        require(responses[4].equals("tick\t5\tA0:" + HexFormat.of().formatHex("Fabric key held P".getBytes(StandardCharsets.UTF_8)) + "\t-\t-\t-\t-"),
                "isDown did not receive the native held-key snapshot");
        String systemMessage = "Fabric system chat message mapped into Horizon";
        require(responses[5].equals("tick\t6\tA0:" + HexFormat.of().formatHex("Fabric forward=true sneak=false forwardPressed=true; sprint enabled; look direction and vertical velocity queued".getBytes(StandardCharsets.UTF_8)) + "\tS1\tV:0.0,0.5,0.0\tM:" + HexFormat.of().formatHex(systemMessage.getBytes(StandardCharsets.UTF_8)) + "\tR:127.5,15.0"),
                "Fabric movement state and press edge did not drive bounded player sprint, velocity, look, system chat, and HUD actions back to Rust");
        require(responses[6].equals("error"), "the bridge accepted a non-finite velocity value");

        require(MAIN_ENTRYPOINTS.get() == 1, "Fabric main initializer was not invoked once");
        require(FabricLoaderImpl.probeStartCount() == 3, "START_CLIENT_TICK listener did not receive three forwarded ticks");
        require(FabricLoaderImpl.probeTickCount() == 3, "END_CLIENT_TICK listener did not receive three forwarded ticks");
        require(FabricLoaderImpl.probeRegisteredKeyCode() == 19, "Fabric key-mapping registration did not preserve its bound key");
        require(MinecraftPlayerSnapshot.read().equals("Tester_2|1.25|64.0|-3.5|1|64|-4|0.0|0.5|0.0|0.5|0.25|0.0|0.0|127.5|15.0|18.5|true|false|6003|303|-64|320|true|false|0.25|0.5|0.2"),
                "the mapped player velocity, sprint, or rotation setters did not update the facade snapshot");
        require(net.minecraft.client.Minecraft.getInstance().player.blockPosition().getX() == 1
                        && net.minecraft.client.Minecraft.getInstance().player.blockPosition().getY() == 64
                        && net.minecraft.client.Minecraft.getInstance().player.blockPosition().getZ() == -4,
                "the mapped block position did not floor negative coordinates like Minecraft");
        net.minecraft.world.level.Level level = net.minecraft.client.Minecraft.getInstance().level;
        require(level.getRainLevel(0.0f) == 0.25f
                        && level.getRainLevel(0.5f) == 0.375f
                        && level.getRainLevel(1.0f) == 0.5f,
                "the rain gradient did not interpolate from the native weather snapshot");
        require(level.getThunderLevel(0.0f) == 0.1f
                        && level.getThunderLevel(0.5f) == 0.15f
                        && level.getThunderLevel(1.0f) == 0.2f,
                "the thunder gradient did not interpolate independently from the native weather snapshot");
        net.minecraft.world.level.block.state.BlockState state = level.getBlockState(
                net.minecraft.client.Minecraft.getInstance().player.blockPosition());
        require("minecraft:stone".equals(state.getBlock().getDescriptionId())
                        && !state.isAir() && state.isSolid() && state.isSolidRender(),
                "the bounded block-state snapshot did not expose its block identity and mapped flags");
        require("minecraft:bedrock".equals(level.getBlockState(new net.minecraft.core.BlockPos(1, 63, -4))
                        .getBlock().getDescriptionId()),
                "the block below the player was not available in the nearby snapshot");
        try {
            level.getBlockState(new net.minecraft.core.BlockPos(4, 64, -4));
            throw new AssertionError("the block facade accepted an uncaptured world position");
        } catch (UnsupportedOperationException expected) {
            // Unknown positions must stay explicit; the adapter cannot query arbitrary Rust world state.
        }
        expectInvalidBlockSnapshot("1,64,-4,minecraft:stone,0,1,1;1,64,-4,minecraft:dirt,0,1,1");
        expectInvalidBlockSnapshot("3,64,-4,minecraft:stone,0,1,1");
        expectInvalidBlockSnapshot("1,64,-4,Minecraft:stone,0,1,1");
        expectInvalidBlockSnapshot("1,64,-4,minecraft:stone,0,2,1");
        expectInvalidBlockSnapshot("1,64,-4," + "x".repeat(9000) + ",0,1,1");
        require(!HorizonFabricBridgeMain.queuePlayerRotation(Float.NaN, 0.0f)
                        && !HorizonFabricBridgeMain.queuePlayerRotation(360_001.0f, 0.0f)
                        && !HorizonFabricBridgeMain.queuePlayerRotation(0.0f, 90.1f),
                "the Java bridge accepted a non-finite or out-of-range player rotation");
        net.minecraft.client.Minecraft.getInstance().player.setXRot(90.1f);
        net.minecraft.client.Minecraft.getInstance().player.setYRot(Float.NaN);
        require(net.minecraft.client.Minecraft.getInstance().player.getYRot() == 127.5f
                        && net.minecraft.client.Minecraft.getInstance().player.getXRot() == 15.0f,
                "invalid rotation setters changed the mapped local-player snapshot");
        System.out.println("Fabric facade contract passed with a test loader stub and an unchanged probe JAR: entrypoints, ordered ticks, player motion/orientation/block-position/world/environment snapshots, namespaced key categories, keyboard registration/isDown/consumeClick, local sprint/velocity/look actions, system chat, action-bar mapping, and unsupported-event rejection.");
    }

    public static void recordMainEntrypoint() {
        MAIN_ENTRYPOINTS.incrementAndGet();
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void expectInvalidBlockSnapshot(String encoded) {
        net.minecraft.client.multiplayer.ClientLevel level = new net.minecraft.client.multiplayer.ClientLevel();
        try {
            level.updateSnapshot(0, 0, -64, 320, false, false, 0, 0, 0, 0,
                    encoded, 1.25, 64, -3.5);
            throw new AssertionError("the Level facade accepted a malformed block snapshot");
        } catch (IllegalArgumentException expected) {
            // Invalid native protocol values must be rejected before the mod callback runs.
        }
    }

    private static final class MinecraftPlayerSnapshot {
        private static String read() {
            net.minecraft.client.Minecraft client = net.minecraft.client.Minecraft.getInstance();
            if (client.player == null) return "none";
            net.minecraft.core.BlockPos block = client.player.blockPosition();
            return client.player.getName().getString() + "|" + client.player.getX() + "|"
                    + client.player.getY() + "|" + client.player.getZ() + "|"
                    + block.getX() + "|" + block.getY() + "|" + block.getZ() + "|"
                    + client.player.getDeltaMovement().x + "|"
                    + client.player.getDeltaMovement().y + "|"
                    + client.player.getDeltaMovement().z + "|"
                    + client.player.getDeltaMovement().length() + "|"
                    + client.player.getDeltaMovement().lengthSqr() + "|"
                    + client.player.getDeltaMovement().horizontalDistance() + "|"
                    + client.player.getDeltaMovement().horizontalDistanceSqr() + "|"
                    + client.player.getYRot() + "|" + client.player.getXRot() + "|"
                    + client.player.getHealth() + "|" + client.player.isSprinting() + "|"
                    + client.player.isOnGround() + "|" + client.level.getDayTime() + "|"
                    + client.level.getGameTime() + "|" + client.level.getMinBuildHeight() + "|"
                    + client.level.getMaxBuildHeight() + "|" + client.level.isRaining() + "|"
                    + client.level.isThundering() + "|" + client.level.getRainLevel(0.0f) + "|"
                    + client.level.getRainLevel(1.0f) + "|" + client.level.getThunderLevel(1.0f);
        }
    }
}
