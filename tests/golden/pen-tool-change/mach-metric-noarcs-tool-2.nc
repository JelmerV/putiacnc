(Summary, written by PutiaCNC)
(Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
(Spindle:                Stays off: the pen draws. Put the paper under it, set Z0 on the)
(                        paper's surface, and make sure the pen is held so that it can lift and)
(                        lower by Z alone.)
(Tools:                  Fine pen 0.5 mm. No tool change.)
(Drawing:                Pen down at Z 0 mm. 45 mm of drawing, about 4 s in all.)
(End:                    Lifts to Z 5 mm, goes to X0 Y0.)
(File 2 of 3: Fine pen 0.5 mm operation 2)
(Before you start: change to Fine pen 0.5 mm and touch off Z0 again.)
(PutiaCNC - Mach3 / LinuxCNC)
(Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G94
G40 G49 G80
G54
G0 Z5
T1 M6 (Tool 1: Fine pen 0.5 mm)
(Pen: the spindle stays off)
(Operation 2: draw with Fine pen 0.5 mm, pen down at 0 mm below Z0, up at 2 mm above it)
G0 X25 Y5
(Pass 1)
G1 Z0 F300
G1 X35 F1500
G1 Y15
G1 X25
G1 Y5
G0 Z2
G0 Z5
G0 X0 Y0
M30