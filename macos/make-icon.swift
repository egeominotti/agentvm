import AppKit
import Foundation

// Draw the project's existing bracket-and-cursor logo in a macOS application icon.
guard CommandLine.arguments.count == 2,
      let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 1024, pixelsHigh: 1024,
          bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
          colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
      let context = NSGraphicsContext(bitmapImageRep: bitmap) else { exit(1) }
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = context
NSColor(calibratedRed: 0.065, green: 0.066, blue: 0.078, alpha: 1).setFill()
NSBezierPath(roundedRect: NSRect(x: 52, y: 52, width: 920, height: 920), xRadius: 200, yRadius: 200).fill()
context.cgContext.translateBy(x: 156, y: 868)
context.cgContext.scaleBy(x: 11.125, y: -11.125)
NSColor(calibratedRed: 0.91, green: 0.925, blue: 0.95, alpha: 1).setStroke()
for points in [[NSPoint(x: 6, y: 20), NSPoint(x: 6, y: 6), NSPoint(x: 20, y: 6)],
               [NSPoint(x: 44, y: 6), NSPoint(x: 58, y: 6), NSPoint(x: 58, y: 20)],
               [NSPoint(x: 58, y: 44), NSPoint(x: 58, y: 58), NSPoint(x: 44, y: 58)],
               [NSPoint(x: 20, y: 58), NSPoint(x: 6, y: 58), NSPoint(x: 6, y: 44)]] {
    let path = NSBezierPath()
    path.lineWidth = 5; path.lineCapStyle = .square
    path.move(to: points[0]); path.line(to: points[1]); path.line(to: points[2]); path.stroke()
}
NSColor(calibratedRed: 0.627, green: 0.549, blue: 1, alpha: 1).setFill()
NSBezierPath(rect: NSRect(x: 22, y: 22, width: 12, height: 20)).fill()
NSColor(calibratedRed: 0.91, green: 0.925, blue: 0.95, alpha: 0.5).setFill()
NSBezierPath(rect: NSRect(x: 38, y: 36, width: 6, height: 6)).fill()
NSGraphicsContext.restoreGraphicsState()
guard let data = bitmap.representation(using: .png, properties: [:]) else { exit(1) }
try data.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
