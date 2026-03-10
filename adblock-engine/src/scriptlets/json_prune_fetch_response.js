(function() {
    const prunePaths = '{{1}}';
    const requiredPaths = '{{2}}';
    const arg3 = '{{3}}';
    const arg4 = '{{4}}';
    const arg5 = '{{5}}';
    const arg6 = '{{6}}';
    if (prunePaths === '' || prunePaths === '{{1}}') return;
    var urlNeedle = '';
    var pairs = [[arg3, arg4], [arg5, arg6]];
    for (var i = 0; i < pairs.length; i++) {
        var key = pairs[i][0], val = pairs[i][1];
        if (key === 'propsToMatch' && val && val.indexOf('{{') !== 0) {
            var parts = val.split(' ');
            for (var j = 0; j < parts.length; j++) {
                if (parts[j].indexOf('url:') === 0) urlNeedle = parts[j].slice(4);
                else urlNeedle = parts[j];
            }
        }
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
    var matchesUrl = function(url) {
        if (!urlNeedle) return true;
        if (isRegex(urlNeedle)) {
            var re = toRegex(urlNeedle);
            return re ? re.test(url) : false;
        }
        return url.includes(urlNeedle);
    };
    var pruneProperty = function(obj, path) {
        if (!obj || typeof obj !== 'object') return;
        if (path.indexOf('[].') === 0 || path.indexOf('[-].') === 0) {
            if (Array.isArray(obj)) {
                var skip = path.charAt(1) === '-' ? 4 : 3;
                for (var i = 0; i < obj.length; i++) {
                    pruneProperty(obj[i], path.slice(skip));
                }
            }
            return;
        }
        var dot = path.indexOf('.');
        if (dot === -1) {
            delete obj[path];
            return;
        }
        var first = path.slice(0, dot);
        var rest = path.slice(dot + 1);
        if (first === '[]' || first === '[-]') {
            if (Array.isArray(obj)) {
                for (var i = 0; i < obj.length; i++) {
                    pruneProperty(obj[i], rest);
                }
            }
        } else if (Array.isArray(obj)) {
            for (var i = 0; i < obj.length; i++) {
                pruneProperty(obj[i], path);
            }
        } else if (obj[first] !== undefined) {
            pruneProperty(obj[first], rest);
        }
    };
    var origFetch = window.fetch;
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
                    try {
                        var json = JSON.parse(text);
                        var paths = prunePaths.split(/\s+/);
                        for (var k = 0; k < paths.length; k++) {
                            if (paths[k]) pruneProperty(json, paths[k]);
                        }
                        return new Response(JSON.stringify(json), {
                            status: response.status,
                            statusText: response.statusText,
                            headers: response.headers
                        });
                    } catch(e) {
                        return new Response(text, {
                            status: response.status,
                            statusText: response.statusText,
                            headers: response.headers
                        });
                    }
                });
            });
        }
    });
})();
