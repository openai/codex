package ai.solace.coder.core.features

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class FeaturesTest {

    @Test
    fun testDefaultFeatures() {
        val features = Features.withDefaults()

        // Stable features should be enabled by default
        assertTrue(features.enabled(Feature.GhostCommit))
        assertTrue(features.enabled(Feature.ViewImageTool))
        assertTrue(features.enabled(Feature.ShellTool))

        // Experimental features should be disabled by default (unless default_enabled)
        assertFalse(features.enabled(Feature.UnifiedExec))
        assertFalse(features.enabled(Feature.RmcpClient))
        assertFalse(features.enabled(Feature.ApplyPatchFreeform))
    }

    @Test
    fun testEnableDisableFeature() {
        val features = Features.withDefaults()

        assertFalse(features.enabled(Feature.UnifiedExec))
        features.enable(Feature.UnifiedExec)
        assertTrue(features.enabled(Feature.UnifiedExec))

        features.disable(Feature.UnifiedExec)
        assertFalse(features.enabled(Feature.UnifiedExec))
    }

    @Test
    fun testFeatureKeyLookup() {
        assertEquals(Feature.GhostCommit, Feature.forKey("undo"))
        assertEquals(Feature.UnifiedExec, Feature.forKey("unified_exec"))
        assertEquals(Feature.ShellTool, Feature.forKey("shell_tool"))
        assertNull(Feature.forKey("unknown_feature"))
    }

    @Test
    fun testLegacyKeyLookup() {
        // Legacy keys should map to new features
        assertEquals(Feature.UnifiedExec, Feature.forKey("use_experimental_unified_exec_tool"))
        assertEquals(Feature.UnifiedExec, Feature.forKey("experimental_use_unified_exec_tool"))
        assertEquals(Feature.ApplyPatchFreeform, Feature.forKey("include_apply_patch_tool"))
    }

    @Test
    fun testIsKnownKey() {
        assertTrue(Feature.isKnownKey("undo"))
        assertTrue(Feature.isKnownKey("unified_exec"))
        assertTrue(Feature.isKnownKey("use_experimental_unified_exec_tool")) // legacy
        assertFalse(Feature.isKnownKey("totally_fake_feature"))
    }

    @Test
    fun testApplyMap() {
        val features = Features.withDefaults()

        val map = mapOf(
            "unified_exec" to true,
            "undo" to false
        )
        features.applyMap(map)

        assertTrue(features.enabled(Feature.UnifiedExec))
        assertFalse(features.enabled(Feature.GhostCommit))
    }

    @Test
    fun testLegacyUsageTracking() {
        val features = Features.withDefaults()

        features.recordLegacyUsage("use_experimental_unified_exec_tool", Feature.UnifiedExec)

        val usages = features.legacyFeatureUsages().toList()
        assertEquals(1, usages.size)
        assertEquals("use_experimental_unified_exec_tool", usages[0].first)
        assertEquals(Feature.UnifiedExec, usages[0].second)
    }

    @Test
    fun testLegacyUsageNotRecordedForCurrentKey() {
        val features = Features.withDefaults()

        // Using the current key shouldn't record legacy usage
        features.recordLegacyUsage("unified_exec", Feature.UnifiedExec)

        val usages = features.legacyFeatureUsages().toList()
        assertEquals(0, usages.size)
    }

    @Test
    fun testFeatureStages() {
        assertEquals(Stage.Stable, Feature.GhostCommit.stage)
        assertEquals(Stage.Experimental, Feature.UnifiedExec.stage)
        assertEquals(Stage.Beta, Feature.ApplyPatchFreeform.stage)
    }

    @Test
    fun testFeatureOverrides() {
        val features = Features.withDefaults()
        assertFalse(features.enabled(Feature.ApplyPatchFreeform))
        assertFalse(features.enabled(Feature.WebSearchRequest))

        val overrides = FeatureOverrides(
            includeApplyPatchTool = true,
            webSearchRequest = true
        )
        overrides.apply(features)

        assertTrue(features.enabled(Feature.ApplyPatchFreeform))
        assertTrue(features.enabled(Feature.WebSearchRequest))
    }

    @Test
    fun testCopy() {
        val features = Features.withDefaults()
        features.enable(Feature.UnifiedExec)

        val copy = features.copy()
        assertTrue(copy.enabled(Feature.UnifiedExec))

        // Modifying copy shouldn't affect original
        copy.disable(Feature.UnifiedExec)
        assertTrue(features.enabled(Feature.UnifiedExec))
        assertFalse(copy.enabled(Feature.UnifiedExec))
    }
}
