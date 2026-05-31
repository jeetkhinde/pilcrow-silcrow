import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
export default function RootLayout({ children }) {
    return (_jsxs("html", { lang: "en", children: [_jsxs("head", { children: [_jsx("meta", { charSet: "UTF-8" }), _jsx("meta", { name: "viewport", content: "width=device-width, initial-scale=1.0" }), _jsx("title", { children: "Pilcrow.js Application" }), _jsx("script", { src: "/_silcrow/silcrow.js", defer: true })] }), _jsx("body", { children: _jsx("div", { id: "app", "data-ps-layout": "/", children: children }) })] }));
}
