(function() {
    const nodeTag = '{{1}}';
    const pattern = '{{2}}';
    const replacement = '{{3}}';
    const arg4 = '{{4}}';
    const arg5 = '{{5}}';
    if (nodeTag === '' || nodeTag === '{{1}}') return;
    if (pattern === '' || pattern === '{{2}}') return;
    const repl = (replacement === '{{3}}') ? '' : replacement;
    var sedCount = Infinity;
    if (arg4 === 'sedCount' && arg5 && arg5 !== '{{5}}') {
        sedCount = parseInt(arg5, 10) || Infinity;
    }
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
    var replaced = 0;
    var replaceInNode = function(node) {
        if (replaced >= sedCount) return;
        var text = node.textContent;
        if (!text) return;
        var modified;
        if (isRegex(pattern)) {
            var re = toRegex(pattern);
            if (!re || !re.test(text)) return;
            modified = text.replace(re, repl);
        } else {
            if (text.indexOf(pattern) === -1) return;
            modified = text.split(pattern).join(repl);
        }
        if (modified !== text) {
            node.textContent = modified;
            replaced++;
        }
    };
    var tagLC = nodeTag.toLowerCase();
    var observer = new MutationObserver(function(mutations) {
        for (var i = 0; i < mutations.length; i++) {
            var added = mutations[i].addedNodes;
            for (var j = 0; j < added.length; j++) {
                var node = added[j];
                if (node.nodeName && node.nodeName.toLowerCase() === tagLC) {
                    replaceInNode(node);
                }
                if (node.querySelectorAll) {
                    var children = node.querySelectorAll(tagLC);
                    for (var k = 0; k < children.length; k++) {
                        replaceInNode(children[k]);
                    }
                }
            }
        }
    });
    observer.observe(document, { childList: true, subtree: true });
    try {
        var existing = document.querySelectorAll(tagLC);
        for (var i = 0; i < existing.length; i++) {
            replaceInNode(existing[i]);
        }
    } catch(e) {}
})();
