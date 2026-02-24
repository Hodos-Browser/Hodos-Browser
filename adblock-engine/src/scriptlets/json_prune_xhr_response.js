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
    var origOpen = XMLHttpRequest.prototype.open;
    var xhrUrls = new WeakMap();
    var xhrModified = new WeakMap();
    var textDescriptor = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'responseText');
    var responseDescriptor = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'response');
    XMLHttpRequest.prototype.open = function(method, url) {
        xhrUrls.set(this, String(url));
        return origOpen.apply(this, arguments);
    };
    var origSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function() {
        var self = this;
        var url = xhrUrls.get(this) || '';
        if (matchesUrl(url)) {
            self.addEventListener('readystatechange', function() {
                if (self.readyState !== 4) return;
                try {
                    var text = textDescriptor.get.call(self);
                    if (!text) return;
                    var json = JSON.parse(text);
                    var paths = prunePaths.split(/\s+/);
                    for (var k = 0; k < paths.length; k++) {
                        if (paths[k]) pruneProperty(json, paths[k]);
                    }
                    var modified = JSON.stringify(json);
                    if (modified !== text) {
                        xhrModified.set(self, modified);
                    }
                } catch(e) {}
            });
        }
        return origSend.apply(this, arguments);
    };
    if (textDescriptor && textDescriptor.get) {
        Object.defineProperty(XMLHttpRequest.prototype, 'responseText', {
            get: function() {
                var mod = xhrModified.get(this);
                if (mod !== undefined) return mod;
                return textDescriptor.get.call(this);
            },
            configurable: true
        });
    }
    if (responseDescriptor && responseDescriptor.get) {
        Object.defineProperty(XMLHttpRequest.prototype, 'response', {
            get: function() {
                var mod = xhrModified.get(this);
                if (mod !== undefined) return mod;
                return responseDescriptor.get.call(this);
            },
            configurable: true
        });
    }
})();
