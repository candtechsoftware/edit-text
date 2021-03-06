// Proxy controller.

import * as route from './route';
import { ControllerImpl } from '../editor/controller';
import { ControllerCommand } from '../bindgen/edit_client';

declare var CONFIG: any;

export class ProxyController implements ControllerImpl {
  onMessage: (msg: any) => void | null;
  onClose: () => void | null;
  onError: (error: any) => void | null;

  private socket: WebSocket;

  sendCommand(command: ControllerCommand) {
    if (CONFIG.console_command_log) {
      console.groupCollapsed('[controller]', command.tag);
      console.debug(command);
      console.groupEnd();
    }

    this.socket.send(JSON.stringify(command));
  }

  connect(): Promise<void> {
    let network = this;
    return Promise.resolve()
    .then(() => {
      this.socket = new WebSocket(
        route.clientProxyUrl()
      );
      this.socket.onopen = function (event) {
        console.debug('client-proxy socket opened.');
      };
      this.socket.onmessage = function (event) {
        let parse = JSON.parse(event.data);
        if (network.onMessage !== null) {
          network.onMessage(parse);
        }
      };
      this.socket.onclose = network.onClose;
    });
  }
}
