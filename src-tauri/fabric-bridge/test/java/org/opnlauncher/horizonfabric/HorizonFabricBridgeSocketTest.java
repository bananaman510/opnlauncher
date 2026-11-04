package org.opnlauncher.horizonfabric;

/** Starts the same bridge entrypoint used by Horizon's Fabric game provider. */
public final class HorizonFabricBridgeSocketTest {
    private HorizonFabricBridgeSocketTest() {}

    public static void main(String[] args) {
        new HorizonFabricGameProvider().launch(HorizonFabricBridgeSocketTest.class.getClassLoader());
    }
}
