import type { JSXInternal } from "preact";

declare global {
  namespace JSX {
    interface IntrinsicElements extends JSXInternal.IntrinsicElements {}
    interface IntrinsicAttributes extends JSXInternal.IntrinsicAttributes {}
    interface Element extends JSXInternal.Element {}
    interface ElementClass extends JSXInternal.ElementClass {}
    interface ElementAttributesProperty extends JSXInternal.ElementAttributesProperty {}
    interface ElementChildrenAttribute extends JSXInternal.ElementChildrenAttribute {}
    interface SVGAttributes extends JSXInternal.SVGAttributes {}
    interface HTMLAttributes extends JSXInternal.HTMLAttributes {}
  }
}
