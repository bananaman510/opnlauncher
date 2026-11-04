package net.minecraft.client.gui;

import net.minecraft.network.chat.Component;
import org.opnlauncher.horizonfabric.HorizonFabricBridgeMain;

/** Maps the vanilla 26.3 HUD action-bar call to Horizon's Rust action bar. */
public final class Hud {
    public void setOverlayMessage(Component message, boolean animate) {
        if (message == null) throw new IllegalArgumentException("HUD message cannot be null");
        HorizonFabricBridgeMain.queueOverlayMessage(message.getString(), animate);
    }
}
