export class AnonRunningError extends Error {
    constructor(message: string) {
        super(message)
        this.name = 'AnonRunningError'
        Object.setPrototypeOf(this, AnonRunningError.prototype)
    }
}
