import axios, { AxiosInstance, AxiosRequestConfig, AxiosResponse } from 'axios';
import { SocksProxyAgent } from 'socks-proxy-agent';
import { Process } from './process';

export class Socks {
  private anon?: Process;
  private agent: SocksProxyAgent;
  public axios: AxiosInstance;
  private socksPort: number;
  private host: string;

  constructor(anon: Process, host?: string);
  constructor(socksPort: number, host?: string);
  constructor(anonOrPort: Process | number, host: string = '127.0.0.1') {
    if (anonOrPort instanceof Process) {
      this.anon = anonOrPort;
      this.socksPort = this.anon.getSOCKSPort();
      console.log('host:', host);
    } else {
      this.socksPort = anonOrPort;
    }

    if (this.socksPort === 0) {
      throw new Error('SOCKS port has value 0, and is therefore disabled.\n' +
        'SOCKS proxy must be enabled with a valid port number (1-65535) to use the SOCKS client');
    }
    
    this.host = host;
    this.agent = this.createAgent();
    this.axios = axios.create({
      httpAgent: this.agent,
      httpsAgent: this.agent
    });
  }

  private createAgent(): SocksProxyAgent {
    return new SocksProxyAgent(`socks://${this.host}:${this.socksPort}`);
  }

  /**
   * Sends a GET request to the specified URL through the Anon network.
   * @param {string} url - The URL to send the GET request to.
   * @param {AxiosRequestConfig} [config] - Optional Axios request configuration.
   * @returns {Promise<AxiosResponse<T>>} A promise that resolves with the response data.
   */
  public get<T = any>(url: string, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.axios.get(url, config);
  }

  /**
   * Sends a POST request to the specified URL through the Anon network.
   * @param {string} url - The URL to send the POST request to.
   * @param {any} [data] - The data to be sent as the request body.
   * @param {AxiosRequestConfig} [config] - Optional Axios request configuration.
   * @returns {Promise<AxiosResponse<T>>} A promise that resolves with the response data.
   */
  public post<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.axios.post(url, data, config);
  }

  /**
   * Sends a PUT request to the specified URL through the Anon network.
   * @param {string} url - The URL to send the PUT request to.
   * @param {any} [data] - The data to be sent as the request body.
   * @param {AxiosRequestConfig} [config] - Optional Axios request configuration.
   * @returns {Promise<AxiosResponse<T>>} A promise that resolves with the response data.
   */
  public put<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.axios.put(url, data, config);
  }

  /**
   * Sends a DELETE request to the specified URL through the Anon network.
   * @param {string} url - The URL to send the DELETE request to.
   * @param {AxiosRequestConfig} [config] - Optional Axios request configuration.
   * @returns {Promise<AxiosResponse<T>>} A promise that resolves with the response data.
   */
  public delete<T = any>(url: string, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.axios.delete(url, config);
  }

  /**
   * Sends a PATCH request to the specified URL through the Anon network.
   * @param {string} url - The URL to send the PATCH request to.
   * @param {any} [data] - The data to be sent as the request body.
   * @param {AxiosRequestConfig} [config] - Optional Axios request configuration.
   * @returns {Promise<AxiosResponse<T>>} A promise that resolves with the response data.
   */
  public patch<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.axios.patch(url, data, config);
  }

}