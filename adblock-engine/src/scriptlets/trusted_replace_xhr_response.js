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
    const origOpen = XMLHttpRequest.prototype.open;
    const xhrUrls = new WeakMap();
    const xhrModified = new WeakMap();
    const textDescriptor = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'responseText');
    const responseDescriptor = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'response');
    XMLHttpRequest.prototype.open = function(method, url) {
        xhrUrls.set(this, String(url));
        return origOpen.apply(this, arguments);
    };
    const origSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function() {
        var self = this;
        var url = xhrUrls.get(this) || '';
        if (matchesUrl(url)) {
            self.addEventListener('readystatechange', function() {
                if (self.readyState !== 4) return;
                try {
                    var text = textDescriptor.get.call(self);
                    if (!text) return;
                    var modified;
                    if (isRegex(pattern)) {
                        var re = toRegex(pattern);
                        modified = re ? text.replace(re, repl) : text;
                    } else {
                        modified = text.split(pattern).join(repl);
                    }
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
