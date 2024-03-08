/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};

use msg::constellation_msg::{TopLevelBrowsingContextId, WebViewId};

#[derive(Debug)]
pub struct WebViewManager<WebView> {
    /// Our top-level browsing contexts. In the WebRender scene, their pipelines are the children of
    /// a single root pipeline that also applies any pinch zoom transformation.
    webviews: HashMap<TopLevelBrowsingContextId, WebView>,

    /// The order in which they were focused, latest last.
    focus_order: Vec<TopLevelBrowsingContextId>,

    /// Whether the latest webview in focus order is currently focused.
    is_focused: bool,

    /// Webviews that are being shown by the compositor, regardless of whether they have been marked as invisible due to
    /// external factors. This set reflects the [compositing_traits::ConstellationMsg::ShowWebView] and
    /// [compositing_traits::ConstellationMsg::HideWebView] messages.
    shown_webviews: HashSet<TopLevelBrowsingContextId>,

    /// Webviews that have been marked as invisible due to external factors, regardless of whether they are being shown
    /// by the compositor. This set reflects the [compositing_traits::ConstellationMsg::MarkWebViewInvisible] and
    /// [compositing_traits::ConstellationMsg::UnmarkWebViewInvisible] messages.
    invisible_webviews: HashSet<TopLevelBrowsingContextId>,
}

impl<WebView> Default for WebViewManager<WebView> {
    fn default() -> Self {
        Self {
            webviews: HashMap::default(),
            focus_order: Vec::default(),
            is_focused: false,
            shown_webviews: HashSet::default(),
            invisible_webviews: HashSet::default(),
        }
    }
}

impl<WebView> WebViewManager<WebView> {
    pub fn add(
        &mut self,
        top_level_browsing_context_id: TopLevelBrowsingContextId,
        webview: WebView,
    ) {
        self.webviews.insert(top_level_browsing_context_id, webview);
    }

    pub fn remove(
        &mut self,
        top_level_browsing_context_id: TopLevelBrowsingContextId,
    ) -> Option<WebView> {
        if self.focus_order.last() == Some(&top_level_browsing_context_id) {
            self.is_focused = false;
        }
        self.focus_order
            .retain(|b| *b != top_level_browsing_context_id);
        self.shown_webviews.remove(&top_level_browsing_context_id);
        self.invisible_webviews
            .remove(&top_level_browsing_context_id);
        self.webviews.remove(&top_level_browsing_context_id)
    }

    pub fn get(
        &self,
        top_level_browsing_context_id: TopLevelBrowsingContextId,
    ) -> Option<&WebView> {
        self.webviews.get(&top_level_browsing_context_id)
    }

    pub fn get_mut(
        &mut self,
        top_level_browsing_context_id: TopLevelBrowsingContextId,
    ) -> Option<&mut WebView> {
        self.webviews.get_mut(&top_level_browsing_context_id)
    }

    pub fn focused_webview(&self) -> Option<(TopLevelBrowsingContextId, &WebView)> {
        if !self.is_focused {
            return None;
        }

        if let Some(top_level_browsing_context_id) = self.focus_order.last().cloned() {
            debug_assert!(
                self.webviews.contains_key(&top_level_browsing_context_id),
                "BUG: webview in .focus_order not in .webviews!",
            );
            self.get(top_level_browsing_context_id)
                .map(|webview| (top_level_browsing_context_id, webview))
        } else {
            debug_assert!(false, "BUG: .is_focused but no webviews in .focus_order!");
            None
        }
    }

    pub fn focus(&mut self, top_level_browsing_context_id: TopLevelBrowsingContextId) {
        debug_assert!(self.webviews.contains_key(&top_level_browsing_context_id));
        self.focus_order
            .retain(|b| *b != top_level_browsing_context_id);
        self.focus_order.push(top_level_browsing_context_id);
        self.is_focused = true;
    }

    pub fn unfocus(&mut self) {
        self.is_focused = false;
    }

    /// Returns true iff the webview’s effective visibility has changed.
    pub fn mark_webview_shown(&mut self, webview_id: WebViewId) -> bool {
        debug_assert!(self.webviews.contains_key(&webview_id));
        let old_effective_visibility = self.is_effectively_visible(webview_id);
        self.shown_webviews.insert(webview_id);
        self.is_effectively_visible(webview_id) != old_effective_visibility
    }

    /// Returns true iff the webview’s effective visibility has changed.
    pub fn mark_webview_not_shown(&mut self, webview_id: WebViewId) -> bool {
        debug_assert!(self.webviews.contains_key(&webview_id));
        let old_effective_visibility = self.is_effectively_visible(webview_id);
        self.shown_webviews.remove(&webview_id);
        self.is_effectively_visible(webview_id) != old_effective_visibility
    }

    /// Returns the set of webviews whose effective visibility has changed.
    pub fn mark_all_webviews_not_shown(&mut self) -> HashSet<WebViewId> {
        let mut result = std::mem::take(&mut self.shown_webviews);
        result.retain(|id| !self.invisible_webviews.contains(id));
        result
    }

    /// Returns true iff the webview’s effective visibility has changed.
    pub fn mark_webview_invisible(&mut self, webview_id: WebViewId) -> bool {
        debug_assert!(self.webviews.contains_key(&webview_id));
        let old_effective_visibility = self.is_effectively_visible(webview_id);
        self.invisible_webviews.insert(webview_id);
        self.is_effectively_visible(webview_id) != old_effective_visibility
    }

    /// Returns true iff the webview’s effective visibility has changed.
    pub fn mark_webview_not_invisible(&mut self, webview_id: WebViewId) -> bool {
        debug_assert!(self.webviews.contains_key(&webview_id));
        let old_effective_visibility = self.is_effectively_visible(webview_id);
        self.invisible_webviews.remove(&webview_id);
        self.is_effectively_visible(webview_id) != old_effective_visibility
    }

    /// Returns true iff the webview is marked as shown and not marked as invisible.
    pub fn is_effectively_visible(&self, webview_id: WebViewId) -> bool {
        debug_assert!(self.webviews.contains_key(&webview_id));
        self.shown_webviews.contains(&webview_id) && !self.invisible_webviews.contains(&webview_id)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::num::NonZeroU32;

    use msg::constellation_msg::{
        BrowsingContextId, BrowsingContextIndex, PipelineNamespace, PipelineNamespaceId,
        TopLevelBrowsingContextId, WebViewId,
    };

    use crate::webview::WebViewManager;

    fn id(namespace_id: u32, index: u32) -> TopLevelBrowsingContextId {
        TopLevelBrowsingContextId(BrowsingContextId {
            namespace_id: PipelineNamespaceId(namespace_id),
            index: BrowsingContextIndex(NonZeroU32::new(index).expect("Incorrect test case")),
        })
    }

    fn ids(ids: impl IntoIterator<Item = (u32, u32)>) -> HashSet<WebViewId> {
        ids.into_iter()
            .map(|(namespace_id, index)| id(namespace_id, index))
            .collect()
    }

    fn webviews_sorted<WebView: Clone>(
        webviews: &WebViewManager<WebView>,
    ) -> Vec<(TopLevelBrowsingContextId, WebView)> {
        let mut keys = webviews.webviews.keys().collect::<Vec<_>>();
        keys.sort();
        keys.iter()
            .map(|&id| {
                (
                    *id,
                    webviews
                        .webviews
                        .get(id)
                        .cloned()
                        .expect("Incorrect test case"),
                )
            })
            .collect()
    }

    #[test]
    fn test() {
        PipelineNamespace::install(PipelineNamespaceId(0));
        let mut webviews = WebViewManager::default();

        // add() adds the webview to the map, but does not focus it.
        webviews.add(TopLevelBrowsingContextId::new(), 'a');
        webviews.add(TopLevelBrowsingContextId::new(), 'b');
        webviews.add(TopLevelBrowsingContextId::new(), 'c');
        assert_eq!(
            webviews_sorted(&webviews),
            vec![(id(0, 1), 'a'), (id(0, 2), 'b'), (id(0, 3), 'c'),]
        );
        assert!(webviews.focus_order.is_empty());
        assert_eq!(webviews.is_focused, false);

        // focus() makes the given webview the latest in focus order.
        webviews.focus(id(0, 2));
        assert_eq!(webviews.focus_order, vec![id(0, 2)]);
        assert_eq!(webviews.is_focused, true);
        webviews.focus(id(0, 1));
        assert_eq!(webviews.focus_order, vec![id(0, 2), id(0, 1)]);
        assert_eq!(webviews.is_focused, true);
        webviews.focus(id(0, 3));
        assert_eq!(webviews.focus_order, vec![id(0, 2), id(0, 1), id(0, 3)]);
        assert_eq!(webviews.is_focused, true);

        // unfocus() clears the “is focused” flag, but does not touch the focus order.
        webviews.unfocus();
        assert_eq!(webviews.focus_order, vec![id(0, 2), id(0, 1), id(0, 3)]);
        assert_eq!(webviews.is_focused, false);

        // focus() avoids duplicates in focus order, when the given webview has been focused before.
        webviews.focus(id(0, 1));
        assert_eq!(webviews.focus_order, vec![id(0, 2), id(0, 3), id(0, 1)]);
        assert_eq!(webviews.is_focused, true);

        webviews.add(id(1, 1), ' ');
        webviews.add(id(1, 2), ' ');
        webviews.mark_webview_invisible(id(1, 2));
        assert_eq!(webviews.shown_webviews, ids([]));
        assert_eq!(webviews.invisible_webviews, ids([(1, 2)]));

        // mark_webview_shown() returns true iff the effective visibility has changed.
        assert_eq!(webviews.mark_webview_shown(id(1, 1)), true); // neither
        assert_eq!(webviews.mark_webview_shown(id(1, 1)), false); // shown
        assert_eq!(webviews.mark_webview_shown(id(1, 2)), false); // invisible
        assert_eq!(webviews.mark_webview_shown(id(1, 2)), false); // both
        assert_eq!(webviews.shown_webviews, ids([(1, 1), (1, 2)]));
        assert_eq!(webviews.invisible_webviews, ids([(1, 2)]));

        webviews.mark_webview_not_shown(id(1, 2));
        webviews.mark_webview_not_invisible(id(1, 2));
        assert_eq!(webviews.shown_webviews, ids([(1, 1)]));
        assert_eq!(webviews.invisible_webviews, ids([]));

        // mark_webview_invisible() returns true iff the effective visibility has changed.
        assert_eq!(webviews.mark_webview_invisible(id(1, 1)), true); // shown
        assert_eq!(webviews.mark_webview_invisible(id(1, 1)), false); // both
        assert_eq!(webviews.mark_webview_invisible(id(1, 2)), false); // neither
        assert_eq!(webviews.mark_webview_invisible(id(1, 2)), false); // invisible
        assert_eq!(webviews.shown_webviews, ids([(1, 1)]));
        assert_eq!(webviews.invisible_webviews, ids([(1, 1), (1, 2)]));

        webviews.mark_webview_shown(id(1, 2));
        webviews.mark_webview_not_invisible(id(1, 2));
        assert_eq!(webviews.shown_webviews, ids([(1, 1), (1, 2)]));
        assert_eq!(webviews.invisible_webviews, ids([(1, 1)]));

        // mark_webview_not_shown() returns true iff the effective visibility has changed.
        assert_eq!(webviews.mark_webview_not_shown(id(1, 1)), false); // both
        assert_eq!(webviews.mark_webview_not_shown(id(1, 1)), false); // invisible
        assert_eq!(webviews.mark_webview_not_shown(id(1, 2)), true); // shown
        assert_eq!(webviews.mark_webview_not_shown(id(1, 2)), false); // neither
        assert_eq!(webviews.shown_webviews, ids([]));
        assert_eq!(webviews.invisible_webviews, ids([(1, 1)]));

        webviews.mark_webview_shown(id(1, 2));
        webviews.mark_webview_invisible(id(1, 2));
        assert_eq!(webviews.shown_webviews, ids([(1, 2)]));
        assert_eq!(webviews.invisible_webviews, ids([(1, 1), (1, 2)]));

        // mark_webview_not_invisible() returns true iff the effective visibility has changed.
        assert_eq!(webviews.mark_webview_not_invisible(id(1, 1)), false); // invisible
        assert_eq!(webviews.mark_webview_not_invisible(id(1, 1)), false); // neither
        assert_eq!(webviews.mark_webview_not_invisible(id(1, 2)), true); // both
        assert_eq!(webviews.mark_webview_not_invisible(id(1, 2)), false); // shown
        assert_eq!(webviews.shown_webviews, ids([(1, 2)]));
        assert_eq!(webviews.invisible_webviews, ids([]));

        // is_effectively_visible() returns true iff the webview is shown and not marked invisible.
        webviews.add(id(2, 1), ' ');
        webviews.add(id(2, 2), ' ');
        webviews.add(id(2, 3), ' ');
        webviews.add(id(2, 4), ' ');
        webviews.mark_webview_shown(id(2, 2));
        webviews.mark_webview_shown(id(2, 4));
        webviews.mark_webview_invisible(id(2, 3));
        webviews.mark_webview_invisible(id(2, 4));
        assert_eq!(webviews.is_effectively_visible(id(2, 1)), false); // neither
        assert_eq!(webviews.is_effectively_visible(id(2, 2)), true); // shown
        assert_eq!(webviews.is_effectively_visible(id(2, 3)), false); // invisible
        assert_eq!(webviews.is_effectively_visible(id(2, 4)), false); // both

        // mark_webview_invisible() does not destroy shown state.
        webviews.add(id(3, 1), ' ');
        webviews.mark_webview_shown(id(3, 1));
        webviews.mark_webview_invisible(id(3, 1));
        webviews.mark_webview_not_invisible(id(3, 1));
        assert_eq!(webviews.is_effectively_visible(id(3, 1)), true);

        // mark_webview_invisible() does not prevent changes to shown state.
        webviews.add(id(4, 1), ' ');
        webviews.mark_webview_invisible(id(4, 1));
        webviews.mark_webview_shown(id(4, 1));
        webviews.mark_webview_not_invisible(id(4, 1));
        assert_eq!(webviews.is_effectively_visible(id(4, 1)), true);

        // remove() clears the “is focused” flag iff the given webview was focused.
        webviews.remove(id(1, 1));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(1, 2));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(2, 1));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(2, 2));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(2, 3));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(2, 4));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(3, 1));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(4, 1));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(0, 2));
        assert_eq!(webviews.is_focused, true);
        webviews.remove(id(0, 1));
        assert_eq!(webviews.is_focused, false);
        webviews.remove(id(0, 3));
        assert_eq!(webviews.is_focused, false);

        // remove() removes the given webview from all data structures.
        assert!(webviews_sorted(&webviews).is_empty());
        assert!(webviews.focus_order.is_empty());
        assert!(webviews.shown_webviews.is_empty());
        assert!(webviews.invisible_webviews.is_empty());
    }
}
