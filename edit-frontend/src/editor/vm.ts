import { Bytecode } from '../bindgen/edit_client';
import * as util from './util';

declare var CONFIG: any;

function assert(condition: boolean) {
    if (!condition) {
        throw new Error('Condition failed.');
    }
}

export function vm(el: Node) {
    let stack: Array<[number, Node]> = [[0, el]];

    let cur = () => { return stack[stack.length - 1]; };
    let curNode = () => {
        assert(cur()[0] <= cur()[1].childNodes.length);
        return cur()[1].childNodes[cur()[0]];
    };
    let lastNode = (): any | null => {
        return cur()[1].childNodes[cur()[0] - 1];
    };

    let handlers: {[value: string]: Function} = {
        Enter() {
            let node = curNode();
            assert(node.nodeType == 1);
            stack.push([0, node]);
        },
        Exit() {
            assert(stack.length > 1); // Can't unenter root
            stack.pop();
            
            // Also perform stack advancement
            cur()[0] += 1;
            assert(cur()[1].childNodes.length >= cur()[0]);
        },
        AdvanceElements(n: number) { // By element
            cur()[0] += n;
            assert(cur()[1].childNodes.length >= cur()[0]);
        },
        DeleteElements(n: number) {
            for (let i = 0; i < n; i++) {
                assert(curNode() !== null);
                curNode().parentNode!.removeChild(curNode());
            }
        },
        InsertDocString([text, styles]: [string, any]) {
            // TODO If this element is following a text node, we just add it
            // to the previous element. right?

            let span = document.createElement('span');
            span.appendChild(document.createTextNode(text));
            Object.keys(styles).map(key => {
                span.classList.add(key);
            });

            // Excessive matching function in JS, where this shouldn't happen
            function hasMatchingTextStyles(left: any, right: any) {
                if (left != null) {
                    if (util.matchesSelector(left, 'span')) {
                        let leftClasses = Array.from(lastNode().classList).sort();
                        let rightClasses = Array.from(span.classList).sort();
    
                        if (leftClasses.join(' ') == rightClasses.join(' ')) {
                            return true;
                        }
                    }
                }
            }

            if (hasMatchingTextStyles(lastNode(), span)) {
                lastNode().append(span.firstChild);
                lastNode().normalize();
                return;
            }
            cur()[1].insertBefore(span, curNode());
            cur()[0] += 1;
        },
        WrapPrevious([n, attrs]: [number, any]) {
            let div = document.createElement('div');
            Object.keys(attrs).forEach(key => {
            	div.setAttribute('data-' + key, attrs[key]);
            });
            cur()[1].insertBefore(div, curNode());
            for (let i = 0; i < n; i++) {
                div.insertBefore(div.previousSibling!, div.firstChild);
            }
            cur()[0] += 1;
        },
        UnwrapSelf() {
            let node = cur()[1];
            stack.pop();
            let children = Array.from(node.childNodes);
            cur()[0] += children.length
            children.forEach(child => node.parentNode!.insertBefore(child, node));
            node.parentNode!.removeChild(node);
        },

        // Take current text node, merge it left, and move on
        JoinTextLeft() {
            let right = curNode();
            assert!(util.matchesSelector(right, 'span'));

            let left = right.previousSibling;
            while (right.childNodes.length) {
                left!.appendChild(right.firstChild!);
            }
            left!.normalize(); // Whoa
            right!.parentNode!.removeChild(right);
        }
    };

    return {
        stack,
        cur,
        curNode,
        
        is_done() {
            return (stack.length == 1 && cur()[0] >= cur()[1].childNodes.length) || stack.length == 0;
        },

        handle(tag: string, fields: any) {
            if (!handlers[tag]) {
                throw new Error(`Unknown opcode ${tag}`)
            }
            // console.warn(tag);
            switch (tag) {
                case 'AdvanceElements':
                    assert(typeof fields == 'number');
                    return handlers.AdvanceElements(fields);
                case 'DeleteElements':
                    assert(typeof fields == 'number');
                    return handlers.DeleteElements(fields);
                case 'InsertDocString':
                    assert(Array.isArray(fields));
                    assert(fields.length == 2);
                    assert(typeof fields[0] == 'string');
                    assert(typeof fields[1] == 'object');
                    return handlers.InsertDocString(fields);
                case 'WrapPrevious':
                    assert(Array.isArray(fields));
                    assert(fields.length == 2);
                    assert(typeof fields[0] == 'number');
                    assert(typeof fields[1] == 'object');
                    return handlers.WrapPrevious(fields);
                default:
                    return handlers[tag]!();
            }
        },

        run(program: Array<Bytecode>) {
            if (CONFIG.console_command_log) {
                console.groupCollapsed('[vm] Script length:', program.length);
            }
            program.forEach((opcode: Bytecode) => {
                if (CONFIG.console_command_log) {
                    console.debug('[vm]', JSON.stringify(opcode));
                }
                this.handle(opcode.tag, 'fields' in opcode ? opcode.fields : opcode);
            });
            if (CONFIG.console_command_log) {
                console.groupEnd();
            }
        }
    };
}
