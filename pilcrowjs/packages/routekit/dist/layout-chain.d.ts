import React from 'react';
export interface LayoutComponentConfig {
    pattern: string;
    component: React.ComponentType<any>;
}
/**
 * Composes a page component and its parent layouts into a single React tree.
 * Wraps layouts in <div data-ps-layout="..."> containers and children in <div data-ps-slot="...">
 * using display: contents to avoid affecting layout styling.
 */
export declare function composeLayoutChain(PageComponent: React.ComponentType<any>, layouts: LayoutComponentConfig[], pagePattern: string, props: any): React.ReactElement;
//# sourceMappingURL=layout-chain.d.ts.map