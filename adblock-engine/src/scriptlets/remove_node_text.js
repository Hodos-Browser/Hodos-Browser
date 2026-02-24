(function() {
    const nodeTag = '{{1}}';
    const pattern = '{{2}}';
    if (nodeTag === '' || nodeTag === '{{1}}') return;
    if (pattern === '' || pattern === '{{2}}') return;
    var isRegex = function(s) {
        if (s.length < 3 || s.charAt(0) !== '/') return false;
        var last = s.lastIndexOf('/');
        return last > 0 && /^[gimsuy]*$/.test(s.slice(last + 1));
    };
    var toRegex = function(s) {
        var m = s.match(/^\/(.+)\/([gimsuy]*)$/);
        if (m) try { return new RegExp(m[1], m[2]); } catch(e) {}
        return null;
    };
    var checkNode = function(node) {
        var text = node.textContent;
        if (!text) return;
        if (isRegex(pattern)) {
            var re = toRegex(pattern);
            if (re && re.test(text)) {
                node.textContent = '';
            }
        } else if (text.indexOf(pattern) !== -1) {
            node.textContent = '';
        }
    };
    var tagLC = nodeTag.toLowerCase();
    var observer = new MutationObserver(function(mutations) {
        for (var i = 0; i < mutations.length; i++) {
            var added = mutations[i].addedNodes;
            for (var j = 0; j < added.length; j++) {
                var node = added[j];
                if (node.nodeName && node.nodeName.toLowerCase() === tagLC) {
                    checkNode(node);
                }
            }
        }
    });
    observer.observe(document, { childList: true, subtree: true });
})();
