(function() {
    const pattern = '{{1}}';
    const replacement = '{{2}}';
    const urlNeedle = '{{3}}';
    if (pattern === '' || pattern === '{{1}}') return;
    const repl = (replacement === '{{2}}') ? '' : replacement;
    const isRegex = function(s) {
        if (s.length < 3 || s.charAt(0) !== '/') return false;
        const last = s.lastIndexOf('/');
        return last > 0 && /^[gimsuy]*$/.test(s.slice(last + 1));
    };
    const toRegex = function(s) {
        const m = s.match(/^\/(.+)\/([gimsuy]*)$/);
        if (m) try { return new RegExp(m[1], m[2]); } catch(e) {}
        return null;
    };
    const matchesUrl = function(url) {
        if (!urlNeedle || urlNeedle === '{{3}}') return true;
        if (isRegex(urlNeedle)) {
            const re = toRegex(urlNeedle);
            return re ? re.test(url) : false;
        }
        return url.includes(urlNeedle);
    };
    const origFetch = window.fetch;
    window.fetch = new Proxy(origFetch, {
        apply: function(target, thisArg, args) {
            var url;
            try {
                url = (args[0] instanceof Request) ? args[0].url : String(args[0]);
            } catch(e) {
                return Reflect.apply(target, thisArg, args);
            }
            if (!matchesUrl(url)) {
                return Reflect.apply(target, thisArg, args);
            }
            return Reflect.apply(target, thisArg, args).then(function(response) {
                var cloned = response.clone();
                return cloned.text().then(function(text) {
                    var modified;
                    try {
                        if (isRegex(pattern)) {
                            var re = toRegex(pattern);
                            modified = re ? text.replace(re, repl) : text;
                        } else {
                            modified = text.split(pattern).join(repl);
                        }
                    } catch(e) {
                        modified = text;
                    }
                    return new Response(modified, {
                        status: response.status,
                        statusText: response.statusText,
                        headers: response.headers
                    });
                });
            });
        }
    });
})();
