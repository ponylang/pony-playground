(function () {
    "use strict";
    const PLAYPEN_URL = "https://playground.ponylang.io";

    /**
     * Returns the `localStorage` item if set or null in case it isn't or an error
     * (like [`SecurityError`](https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage#securityerror)) is thrown
     * @param {String} key 
     * @returns {String|null}
     */
    function optionalLocalStorageGetItem(key) {
        try {
            return localStorage.getItem(key);
        } catch (e) {
            return null;
        }
    }

    /**
     * Sets the `localStorage` item and ignores potential exceptions
     * (like [`QuotaExceededError`](https://developer.mozilla.org/en-US/docs/Web/API/Storage/setItem#quotaexceedederror)) is thrown
     * @param {String} key 
     * @param {String} value
     * @returns {void}
     */
    function optionalLocalStorageSetItem(key, value) {
        try {
            window.localStorage.setItem(key, value);
        } catch (e) {
            // ignore
        }
    }

    /**
     * Creates [`<option>`s](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/option)
     * for the `#themes` select
     * @param {Object} themelist 
     * @returns {void}
     */
    function build_themes(themelist) {
        // Load all ace themes, sorted by their proper name.
        const themes = themelist.themes;
        themes.sort((a, b) => a.caption.localeCompare(b.caption));

        let themeopt,
            themefrag = document.createDocumentFragment();
        for (const theme of themes) {
            themeopt = document.createElement("option");
            themeopt.setAttribute("val", theme.theme);
            themeopt.textContent = theme.caption;
            themefrag.appendChild(themeopt);
        }
        document.getElementById("themes").appendChild(themefrag);
    }

    /**
     * Sends an HTTP request and writes the result to the #result element
     * @param {string} path 
     * @param {any} data 
     * @param {Function} callback 
     * @param {HTMLButtonElement} button 
     * @param {String} message 
     * @param {HTMLDivElement} result 
     * @returns {void}
     */
    async function send(path, data, callback, button, message, result) {
        let response, json;
        button.disabled = true;

        set_result(result, "<p class=message>" + message);

        try {
            response = await fetch(path, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                },
                signal: AbortSignal.timeout(5000),
                body: JSON.stringify(data),
            });
        } catch(err) {
            if ((err instanceof DOMException) && err.name === "AbortError") {
                set_result(window, result, "<p class=error>Connection timed out" +
                "<p class=error-explanation>Are you connected to the Internet?");
            } else {
                set_result(result, "<p class=error>Something went wrong" +
                        "<p class=error-explanation>The HTTP request produced an error with message " + err.message + ".");
            }
        }
        button.disabled = false;

        try {
            json = await response.json();
        } catch (e) {
            console.log("JSON.parse(): " + e);
        }

        if (response.status === 200) {
            callback(json);
        } else if (response.status === 0) {
            set_result(result, "<p class=error>Connection failure" +
                        "<p class=error-explanation>Are you connected to the Internet?");
        } else {
            set_result(result, "<p class=error>Something went wrong" +
                        "<p class=error-explanation>The HTTP request produced a response with status code " + response.status + ".");
        }
    }

    const PYGMENTS_TO_ACE_MAPPINGS = {
        'asm': {
            'c': 'ace_comment', // Comment,
            'na': 'ace_support ace_function ace_directive', // Name.Attribute,
            'no': 'ace_constant', // Name.Constant,
            'nl': 'ace_entity ace_name ace_function', // Name.Label,
            'nv': 'ace_variable ace_parameter ace_register', // Name.Variable,
            'mh': 'ace_constant ace_character ace_hexadecimal', // Number.Hex,
            'mi': 'ace_constant ace_character ace_decimal', // Number.Integer,
            'p': 'ace_punctuation', // Punctuation,
            's': 'ace_string', // String,
            'sc': 'ace_string', // String.Char,
            '': '', // Text,
        },
        'llvm-ir': {
            'c': 'ace_comment', // Comment
            'k': 'ace_keyword', // Keyword
            'kt': 'ace_storage ace_type', // Keyword.Type
            'nl': 'ace_identifier', // Name.Label
            'nv': 'ace_variable', // Name.Variable
            'nv-Anonymous': 'ace_support ace_variable', // Name.Variable.Anonymous
            'vg': 'ace_variable ace_other', // Name.Variable.Global
            'm': 'ace_constant ace_numeric', // Number
            'p': 'ace_punctuation', // Punctuation
            's': 'ace_string', // String
            '': '', // Text
        }
    };

    /**
     * Refresh the syntax highlighting via pygments
     * @param {String} pygmentized 
     * @param {keyof PYGMENTS_TO_ACE_MAPPINGS} language 
     * @returns {String}
     */
    function rehighlight(pygmentized, language) {
        const mappings = PYGMENTS_TO_ACE_MAPPINGS[language];
        return pygmentized.replace(/<span class="([^"]*)">([^<]*)<\/span>/g, function () {
            const classes = mappings[arguments[1]];
            if (classes) {
                return '<span class="' + classes + '">' + arguments[2] + '</span>';
            }
            return arguments[2];
        });
    }

    function redrawResult(result) { // @todo not used anymore? Remove?
        // Sadly the fun letter-spacing animation can leave artefacts,
        // so we want to manually trigger a redraw. It doesn’t matter
        // whether it’s relative or static for now, so we’ll flip that.
        result.parentNode.style.visibility = "hidden";
        var _ = result.parentNode.offsetHeight;  // This empty assignment is intentional
        result.parentNode.style.visibility = "";
    }

    /**
     * Passes the code to `send()` and displays the evaluated code in `#result`
     * @param {HTMLDivElement} result 
     * @param {String} code 
     * @param {HTMLButtonElement} button 
     * @return {void}
     */
    function evaluate(result, code, button) {
        send("/evaluate.json", {
            code: code,
            separate_output: true,
            color: true,
            branch: branch
        }, function (object) {
            let samp, pre, h;
            set_result(result);
            if (object.compiler) {
                h = document.createElement("span");
                h.className = "output-header";
                h.textContent = "Ponyc Output";
                result.appendChild(h);

                samp = document.createElement("samp");
                samp.innerHTML = formatCompilerOutput(object.compiler);
                pre = document.createElement("pre");
                pre.classList.add("ponyc-output");
                pre.classList.add("output");
                pre.appendChild(samp);
                result.appendChild(pre);
            }

            if (object.stdout) {
                h = document.createElement("span");
                h.className = "output-header";
                h.textContent = "Standard Output";
                result.appendChild(h);

                samp = document.createElement("samp");
                samp.className = "output-stdout";
                pre.classList.add("output");
                samp.innerHTML = formatCompilerOutput(object.stdout);
                pre = document.createElement("pre");
                pre.appendChild(samp);
                result.appendChild(pre);
            }
            if (object.stderr) {
                h = document.createElement("span");
                h.className = "output-header";
                pre.classList.add("output");
                h.textContent = "Standard Error";
                result.appendChild(h);

                samp = document.createElement("samp");
                samp.className = "output";
                samp.innerHTML = formatCompilerOutput(object.stderr);
                pre = document.createElement("pre");
                pre.classList.add("output");
                pre.classList.add("stderr");
                pre.appendChild(samp);
                result.appendChild(pre);
            }

            const div = document.createElement("p");
            div.className = "message";
            if (object.success && (object.stdout || object.stderr)) {
                div.textContent = "Program ended.";
            } else if (object.success) {
                div.textContent = "Program ended with no output.";
            } else {
                div.textContent = "Compilation failed.";
            }
            result.appendChild(div);
        }, button, "Running…", result);
    }

    /**
     * Passes the code to `send()` and displays the compiled code in `#result`
     * @param {'asm'|'llvm-ir'} emit
     * @param {HTMLDivElement} result 
     * @param {String} code 
     * @param {HTMLButtonElement} button 
     * @return {void}
     */
    function compile(emit, result, code, button) {
        send("/compile.json", {
            emit: emit,
            code: code,
            color: true,
            highlight: true,
            branch: branch
        }, function (object) {
            if ("error" in object) {
                set_result(result, "<pre class=\"rustc-output rustc-errors\"><samp></samp></pre>");
                result.firstElementChild.firstElementChild.innerHTML = formatCompilerOutput(object.error);
            } else {
                set_result(result, "<pre class=highlight><code>" + rehighlight(object.result, emit) + "</code></pre>");
            }
        }, button, "Compiling…", result);
    }

    /**
     * Creates a gist for the current code via `send()` and
     * displays both the gist link and playground permalink in `#result`
     * @param {'asm'|'llvm-ir'} emit
     * @param {HTMLDivElement} result 
     * @param {String} code 
     * @param {HTMLButtonElement} button 
     * @return {void}
     */
    function shareGist(result, code, button) {
        send("/gist.json", {
            code: code,
            base_url: PLAYPEN_URL,
            branch: branch,
        }, function (response) {
            const gist_id = response.gist_id;
            const gist_url = response.gist_url;

            const play_url = PLAYPEN_URL + "/?gist=" + encodeURIComponent(gist_id);

            if (branch !== "release") {
                play_url += "&branch=" + branch;
            }

            set_result(
                result,
                "<p><a href=" + play_url + ">Permalink to the playground</a></p>" +
                "<p><a href=" + gist_url + ">Direct link to the gist</a></p>"
            );
        }, button, "Creating Gist…", result);
    }

    /**
     * 
     * @param {'GET'|'HEAD'|'POST'|'PUT'|'DELETE'|'CONNECT'|'OPTIONS'|'TRACE'|'PATCH'} method (see [HTTP request methods](https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods))
     * @param {String} url 
     * @param {any} data 
     * @param {Number} expect 
     * @param {Function} on_success 
     * @param {Function} on_fail 
     * @return {void}
     */
    async function httpRequest(method, url, data, expect, on_success, on_fail) {
        const options = {
            method,          
        };
        if (method === 'POST') {
            options.body = data;
        }
        const response = await fetch(url, options);
        if (response.status == expect && on_success instanceof Function) {
            on_success(response);
        }
        if (response.status != expect && on_fail instanceof Function) {
            on_fail(response.status, response);
        }
      }

    /**
     * Fetches a gist from the [gist.github.com](https://gist.github.com/) and loads it into the code editor
     * @param {Ace.EditSession} session (see [ace.Ace.Editor::getSession](https://ajaxorg.github.io/ace-api-docs/interfaces/ace.Ace.Editor.html#getSession))
     * @param {HTMLDivElement} result 
     * @param {String} gist_id 
     * @param {bool} do_evaluate 
     * @param {HTMLButtonElement} evaluateButton 
     * @return {void}
     */
    function fetchGist(session, result, gist_id, do_evaluate, evaluateButton) { // @todo is the evaluateButton used globally here? We should choose if we want to pass it or use it locally but not mix both options
        session.setValue("// Loading Gist: https://gist.github.com/" + gist_id + " ...");
        httpRequest("GET", "https://api.github.com/gists/" + gist_id, null, 200,
            async function (response) {
                response = await response.json();
                if (!response) {
                    return;
                }

                const files = response.files;
                for (const [ name, file ] of Object.entries(files)) {
                    if (!files.hasOwnProperty(name)) {
                        continue;
                    }

                    session.setValue(file.content);

                    if (do_evaluate) {
                        doEvaluate();
                    }
                    break;
                }
            },
            function (status, response) {
                set_result(result, "<p class=error>Failed to fetch Gist" +
                    "<p class=error-explanation>Are you connected to the Internet?");
            }
        );
    }

    /**
     * Fetches a code snippet from the [`ponylang/pony-tutorial`](https://github.com/ponylang/pony-tutorial) repository on GitHub
     * and loads it into the code editor
     * @param {Ace.EditSession} session (see [ace.Ace.Editor::getSession](https://ajaxorg.github.io/ace-api-docs/interfaces/ace.Ace.Editor.html#getSession))
     * @param {HTMLDivElement} result 
     * @param {String} gist_id 
     * @param {bool} do_evaluate 
     * @param {HTMLButtonElement} evaluateButton 
     * @return {void}
     */
    function fetchSnippet(session, result, snippet_file_name, do_evaluate, evaluateButton, docsButton) {
        session.setValue("// Loading snippet: https://github.com/ponylang/pony-tutorial/blob/main/code-samples/" + snippet_file_name + " ...");
        httpRequest("GET", "https://raw.githubusercontent.com/ponylang/pony-tutorial/main/code-samples/" + snippet_file_name, null, 200,
            function (response) {
                session.setValue(response);

                if (query.has('docs')) {
                    const docsUrl = `https://tutorial.ponylang.io/${query.get('docs')}`
                    fetch(docsUrl)
                        .then(res => res.text())
                        .then(htmlString => (new DOMParser()).parseFromString(htmlString, "text/html"))
                        .then(htmlDocument => {
                            docsButton.querySelector('output').textContent = htmlDocument.querySelector('head > title').textContent
                        })
                    docsButton.removeAttribute('hidden')
                    docsButton.querySelector('output').textContent = query.get('docs') // placeholder, until title is loaded
                    docsButton.href = docsUrl
                }

                if (do_evaluate) {
                    doEvaluate();
                }
            },
            function (status, response) {
                set_result(result, "<p class=error>Failed to fetch snippet" +
                    "<p class=error-explanation>Are you connected to the Internet?");
            }
        );
    }

    /**
     * Get URL query parameters as Object
     * @returns {URLSearchParams}
     */
    function getQueryParameters() {
        const url = new URL(window.location);
        return url.searchParams;
    }

    /**
     * Clears the result in various places
     * @param {HTMLDivElement} result 
     * @returns {void}
     */
    function clear_result(result) {
        result.innerHTML = "";
        result.parentNode.dataset.empty = "";
        set_result.editor.resize();
    }

    /**
     * Sets the content of `#result` and resizes the editor
     * @param {HTMLDivElement} result 
     * @param {undefined|string|HTMLElement} contents 
     * @returns {void}
     */
    function set_result(result, contents) {
        delete result.parentNode.dataset.empty;
        if (contents === undefined) {
            result.textContent = "";
        } else if (typeof contents === "string") {
            result.innerHTML = contents;
        } else {
            result.textContent = "";
            result.appendChild(contents);
        }
        set_result.editor.resize();
    }

    /**
     * Calls `setKeyboardHandler()` on `Ace.editor`
     * @param {Ace.editor} editor (see [Ace.editor](https://ajaxorg.github.io/ace-api-docs/interfaces/ace.Ace.Editor.html))
     * @param {'Ace'|'Emacs'|'Vim'} mode 
     * @returns {void}
     */
    function set_keyboard(editor, mode) {
        if (mode === "Emacs") {
            editor.setKeyboardHandler("ace/keyboard/emacs");
        } else if (mode === "Vim") {
            editor.setKeyboardHandler("ace/keyboard/vim");
            if (!set_keyboard.vim_set_up) {
                ace.config.loadModule("ace/keyboard/vim", function (m) {
                    const Vim = ace.require("ace/keyboard/vim").CodeMirror.Vim;
                    Vim.defineEx("write", "w", function (cm, input) {
                        cm.ace.execCommand("evaluate");
                    });
                });
            }
            set_keyboard.vim_set_up = true;
        } else {
            editor.setKeyboardHandler(null);
        }
    }

    /**
     * Sets the theme on `Ace.editor`
     * @param {Ace.editor} editor (see [Ace.editor](https://ajaxorg.github.io/ace-api-docs/interfaces/ace.Ace.Editor.html))
     * @param {Object} themelist 
     * @param {String} theme 
     * @returns {void}
     */
    function set_theme(editor, themelist, theme) {
        const themes = document.getElementById("themes");
        let themepath = null,
            selected = themes.options[themes.selectedIndex];
        if (selected.textContent === theme) {
            themepath = selected.getAttribute("val");
        } else {
            for (const [ i, currentTheme ] of themelist.themes.entries()) {
                if (currentTheme.caption == theme) {
                    themes.selectedIndex = i;
                    themepath = currentTheme.theme;
                    break;
                }
            }
        }
        if (themepath !== null) {
            editor.setTheme(themepath);
            optionalLocalStorageSetItem("theme", theme);
        }
    }

    let evaluateButton; // @todo is the evaluateButton used globally here? We should choose if we want to pass it or use it locally but not mix both options
    let asmButton;
    let irButton;
    let gistButton;
    let docsButton;
    let configureEditorButton;
    let result;
    let clearResultButton;
    let keyboard;
    let themes;
    let editor;
    let session;
    let themelist;
    let theme;
    let mode;
    let query;

    function doEvaluate() {
        var code = session.getValue(); // @todo not used anymore? Remove?
        evaluate(result, session.getValue(), evaluateButton); // @todo is the evaluateButton used globally here? We should choose if we want to pass it or use it locally but not mix both options
    }

    const COLOR_CODES = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];

    /**
     * A simple function to decode ANSI escape codes into HTML.
     * This is very basic, with lots of very obvious omissions and holes;
     * it’s designed purely to cope with rustc output.
     * 
     * TERM=xterm rustc uses these:
     * - bug/fatal/error = red
     * - warning = yellow
     * - note = green
     * - help = cyan
     * - error code = magenta
     * - bold
     * 
     * @param {String} text 
     * @returns {String}
     */
    function ansi2html(text) {
        return text.replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/\x1b\[1m\x1b\[3([0-7])m([^\x1b]*)(?:\x1b\(B)?\x1b\[0?m/g, function (original, colorCode, text) {
                return '<span class=ansi-' + COLOR_CODES[+colorCode] + '><strong>' + text + '</strong></span>';
            }).replace(/\x1b\[3([0-7])m([^\x1b]*)(?:\x1b\(B)?\x1b\[0?m/g, function (original, colorCode, text) {
                return '<span class=ansi-' + COLOR_CODES[+colorCode] + '>' + text + '</span>';
            }).replace(/\x1b\[1m([^\x1b]*)(?:\x1b\(B)?\x1b\[0?m/g, function (original, text) {
                return "<strong>" + text + "</strong>";
            }).replace(/(?:\x1b\(B)?\x1b\[0?m/g, '');
    }

    /**
     * This affects how mouse acts on the program output.
     * Screenshots here: https://github.com/rust-lang/rust-playpen/pull/192#issue-145465630
     * If mouse hovers on eg. "<anon>:3", temporarily show that line(3) into view by
     * selecting it entirely and move editor's cursor to the beginning of it;
     * Moves back to original view when mouse moved away.
     * If mouse left click on eg. "<anon>:3" then the editor's cursor is moved
     * to the beginning of that line
     * 
     * @param {String} text 
     * @param {Number} r1 Line number
     */
    function jumpToLine(text, r1) { // @todo not used anymore? Remove?
        return "<a onclick=\"javascript:editGo(" + r1 + ",1)\"" +
            " onmouseover=\"javascript:editShowLine(" + r1 + ")\"" +
            " onmouseout=\"javascript:editRestore()\"" +
            " class=\"linejump\">" + text + "</a>";
    }

    /*
     * Similarly to `jumpToLine()`, except this one acts on eg. "<anon>:2:31: 2:32"
     * and thus selects a region on mouse hover, or when clicked sets cursor to
     * the beginning of region.
     * 
     * @param {String} text 
     * @param {Number} r1 Line start number
     * @param {Number} c1 Column start number
     * @param {Number} r2 Line end number
     * @param {Number} c2 Column end number
     */
    function jumpToRegion(text, r1, c1, r2, c2) { // @todo not used anymore? Remove?
        return "<a onclick=\"javascript:editGo(" + r1 + "," + c1 + ")\"" +
            " onmouseover=\"javascript:editShowRegion(" + r1 + "," + c1 + ", " + r2 + "," + c2 + ")\"" +
            " onmouseout=\"javascript:editRestore()\"" +
            " class=\"linejump\">" + text + "</a>";
    }

    /**
     * Similarly to `jumpToLine()`, except this one acts on eg. "<anon>:2:31"
     * 
     * @param {String} text 
     * @param {Number} r1 Line number
     * @param {Number} c1 Column number
     */
    function jumpToPoint(text, r1, c1) {
        return "<a onclick=\"javascript:editGo(" + r1 + "," + c1 + ")\"" +
            " onmouseover=\"javascript:editShowPoint(" + r1 + "," + c1 + ")\"" +
            " onmouseout=\"javascript:editRestore()\"" +
            " class=\"linejump\">" + text + "</a>";
    }

    /**
     * Replaces paths and adds jump links
     * @param {String} text 
     * @returns {String}
     */
    function formatCompilerOutput(text) {
        return ansi2html(text)
            .replace(/\/.*\/main.pony/mg, "main.pony")
            .replace(/\/usr\/local\/lib\/pony\/[^\/]*\//mg, "")
            .replace(/main\.pony:(\d+):(\d+)/mg, jumpToPoint);
    }

    addEventListener("DOMContentLoaded", function () {
        evaluateButton = document.getElementById("evaluate");
        asmButton = document.getElementById("asm");
        irButton = document.getElementById("llvm-ir");
        gistButton = document.getElementById("gist");
        docsButton = document.getElementById("docs");
        configureEditorButton = document.getElementById("configure-editor");
        result = document.getElementById("result").firstElementChild;
        clearResultButton = document.getElementById("clear-result");
        keyboard = document.getElementById("keyboard");
        themes = document.getElementById("themes");
        editor = ace.edit("editor");
        set_result.editor = editor;
        editor.$blockScrolling = Infinity;
        editor.setAnimatedScroll(true);
        session = editor.getSession();
        themelist = ace.require("ace/ext/themelist");
        window.branch = "release"

        editor.focus();

        build_themes(themelist);

        editor.renderer.on('themeChange', function (e) {
            var path = e.theme;
            ace.config.loadModule(['theme', e.theme], function (t) {
                const resultEl = document.getElementById("result");
                resultEl.className = t.cssClass;
                if (t.isDark) {
                    resultEl.classList.add("ace_dark");
                }
            });
        });

        theme = optionalLocalStorageGetItem("theme");
        set_theme(editor, themelist, theme ?? "GitHub");

        session.setMode("ace/mode/pony");

        // Match the tab style of the Pony standard library.
        session.setTabSize(2);
        session.setUseSoftTabs(true);

        mode = optionalLocalStorageGetItem("keyboard");
        if (mode !== null) {
            set_keyboard(editor, mode);
            keyboard.value = mode;
        }

        query = getQueryParameters();
        if (query.has("code")) {
            session.setValue(query.get("code"));
        } else if (query.has("gist")) {
            // fetchGist() must defer evaluation until after the content has been loaded
            fetchGist(session, result, query.get("gist"), query.get("run") === "1", evaluateButton);
            query.set("run", 0);
        } else if (query.has("snippet")) {
            // fetchSnippet() must defer evaluation until after the content has been loaded
            fetchSnippet(session, result, query.get("snippet"), query.get("run") === "1", evaluateButton, docsButton);
            query.set("run", 0);
        } else {
            var code = optionalLocalStorageGetItem("code");
            if (code !== null) {
                session.setValue(code);
            }
        }

        if (query.has("branch")) {
            branch = query.get("branch")
        }

        if (query.get("run") === "1") {
            doEvaluate();
        }

        addEventListener("resize", editor.resize);

        //This helps re-focus editor after a Run or any other action that caused
        //editor to lose focus. Just press Enter or Esc key to focus editor.
        //Without this, you'd most likely have to LMB somewhere in the editor
        //area which would change the location of its cursor to where you clicked.
        addEventListener("keyup", function (e) {
            if ((document.body == document.activeElement) && //needed to avoid when editor has focus already
                (["Enter", "Escape"].includes(e.code))) { //Enter or Escape keys
                editor.focus();
            }
        });

        session.on("change", function () {
            var code = session.getValue();
            optionalLocalStorageSetItem("code", code);
        });

        keyboard.onkeyup = keyboard.onchange = function () {
            var mode = keyboard.options[keyboard.selectedIndex].value;
            optionalLocalStorageSetItem("keyboard", mode);
            set_keyboard(editor, mode);
        };

        evaluateButton.onclick = function () {
            doEvaluate(true);
            editor.focus();
        };

        editor.commands.addCommand({
            name: "evaluate",
            exec: evaluateButton.onclick,
            bindKey: { win: "Ctrl-Enter", mac: "Ctrl-Enter" }
        });

        // ACE uses the "cstyle" behaviour for all languages by default, which
        // gives us nice things like quote and paren autopairing. However this
        // also autopairs single quotes, which makes writing lifetimes annoying.
        // To avoid having to duplicate the other functionality provided by the
        // cstyle behaviour, we work around this situation by hijacking the
        // single quote as a hotkey and modifying the document ourselves, which
        // does not trigger this behaviour.
        editor.commands.addCommand({
            name: "rust_no_single_quote_autopairing",
            exec: function (editor, line) {
                const sess = editor.getSession();
                const doc = sess.getDocument();
                const selection = sess.getSelection();
                const ranges = selection.getAllRanges();
                let prev_range = null;

                // no selection = zero width range, so we don't need to handle this case specially
                // start from the back, so changes to earlier ranges don't invalidate later ranges
                for (const i = ranges.length - 1; i >= 0; i--) {
                    // sanity check: better to do no modification than to do something wrong
                    // see the compareRange docs:
                    // https://github.com/ajaxorg/ace/blob/v1.2.6/lib/ace/range.js#L106-L120
                    if (prev_range && prev_range.compareRange(ranges[i]) != -2) {
                        console.log("ranges intersect or are not in ascending order, skipping",
                            ranges[i]);
                    }
                    prev_range = ranges[i];

                    doc.replace(ranges[i], "'");
                }
                // the selection contents were replaced, so clear the selection
                selection.clearSelection();
            },
            bindKey: { win: "'", mac: "'" },
        });

        // We’re all pretty much agreed that such an obscure command as transposing
        // letters hogging Ctrl-T, normally “open new tab”, is a bad thing.
        const transposeletters = editor.commands.commands.transposeletters;
        editor.commands.removeCommand("transposeletters");
        delete transposeletters.bindKey;
        editor.commands.addCommand(transposeletters);

        asmButton.onclick = compile.bind(window, "asm", result, session.getValue(), asmButton);

        irButton.onclick = compile.bind(window, "llvm-ir", result, session.getValue(), irButton);

        gistButton.onclick = shareGist.bind(window, result, session.getValue(), gistButton);

        configureEditorButton.onclick = function () {
            const dropdown = configureEditorButton.nextElementSibling;
            dropdown.style.display = dropdown.style.display ? "" : "block";
        };

        clearResultButton.onclick = clear_result.bind(window, result);

        themes.onkeyup = themes.onchange = set_theme.bind(window, editor, themelist, themes.options[themes.selectedIndex].text);
    }, false);
}());


// called via javascript:fn events from formatCompilerOutput
var old_range;

/**
 * Get an instance of `Ace.editor` on `#editor`
 * @returns {Ace.editor} (see [Ace.editor](https://ajaxorg.github.io/ace-api-docs/interfaces/ace.Ace.Editor.html))
 */
function editorGet() {
    return window.ace.edit("editor");
}

/**
 * Selects text at the given line and column
 * @param {Number} r1 Line number
 * @param {Number} c1 Column number
 * @returns {void}
 */
function editGo(r1, c1) {
    const e = editorGet();
    old_range = undefined;
    e.focus();
    e.selection.clearSelection();
    e.scrollToLine(r1 - 1, true, true);
    e.selection.moveCursorTo(r1 - 1, c1 - 1, false);
}

function editRestore() {
    if (!old_range) {
        return
    }

    const e = editorGet();
    e.selection.setSelectionRange(old_range, false);
    const mid = (e.getFirstVisibleRow() + e.getLastVisibleRow()) / 2;
    const intmid = Math.round(mid);
    const extra = (intmid - mid) * 2 + 2;
    const up = e.getFirstVisibleRow() - old_range.start.row + extra;
    const down = old_range.end.row - e.getLastVisibleRow() + extra;
    if (up > 0) {
        e.scrollToLine(mid - up, true, true);
    } else if (down > 0) {
        e.scrollToLine(mid + down, true, true);
    } // else visible enough
}

/**
 * Selects text at the given line and region
 * @param {Number} r1 Line start number
 * @param {Number} c1 Column start number
 * @param {Number} r2 Line end number
 * @param {Number} c2 Column end number
 * @returns {void}
 */
function editShowRegion(r1, c1, r2, c2) {
    const e = editorGet();
    const es = e.selection;
    old_range = es.getRange();
    es.clearSelection();
    e.scrollToLine(Math.round((r1 + r2) / 2), true, true);
    es.setSelectionAnchor(r1 - 1, c1 - 1);
    es.selectTo(r2 - 1, c2 - 1);
}

/**
 * Selects text at the given line
 * @param {Number} r1 Line number
 * @returns {void}
 */
function editShowLine(r1) {
    const e = editorGet();
    const es = e.selection;
    old_range = es.getRange();
    es.clearSelection();
    e.scrollToLine(r1, true, true);
    es.moveCursorTo(r1 - 1, 0);
    es.moveCursorLineEnd();
    es.selectTo(r1 - 1, 0);
}

/**
 * Selects text at the given line and column
 * @param {Number} r1 Line number
 * @param {Number} c1 Column number
 * @returns {void}
 */
function editShowPoint(r1, c1) {
    const e = editorGet();
    const es = e.selection;
    old_range = es.getRange();
    es.clearSelection();
    e.scrollToLine(r1, true, true);
    es.moveCursorTo(r1 - 1, 0);
    es.moveCursorLineEnd();
    es.selectTo(r1 - 1, c1 - 1);
}
