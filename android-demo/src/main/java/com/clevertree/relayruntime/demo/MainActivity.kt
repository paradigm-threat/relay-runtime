package com.clevertree.relayruntime.demo

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity

/**
 * Demo: uses HookRenderer as the main rendering component.
 * HookRenderer loads hooks from assets, runs via relay-runtime, renders VNode→View.
 */
class MainActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val hookRenderer = HookRenderer(this).apply {
            hookPath = "hooks/app.jsx"
            layoutParams = android.view.ViewGroup.LayoutParams(
                android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                android.view.ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        setContentView(hookRenderer)
        hookRenderer.runRender()
    }
}
