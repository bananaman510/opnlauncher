package net.minecraft.network.chat;

/** Plain-text subset of Minecraft 26.3 Component used by the HUD mapping. */
public interface Component {
    String getString();

    static MutableComponent literal(String value) {
        return new MutableComponent(value);
    }
}
