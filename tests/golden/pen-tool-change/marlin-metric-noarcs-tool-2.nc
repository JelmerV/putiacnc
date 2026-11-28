(Summary, written by PutiaCNC)
(Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
(                        speed shown, and again before a tool change and at the end to stop it.)
(                        There is no work coordinate system: jog to the zero of the job and set)
(                        it with G92 X0 Y0 Z0 first.)
(Tools:                  Fine pen 0.5 mm. No tool change.)
(Cutting:                Down to Z 0 mm. 45 mm of cutting, about 4 s in all.)
(End:                    Lifts to Z 5 mm, goes to X0 Y0.)
(File 2 of 3: Fine pen 0.5 mm operation 2)
(Before you start: change to Fine pen 0.5 mm and touch off Z0 again.)
; PutiaCNC - Marlin (no spindle control)
; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
G21 G90
; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
; Spindle: by hand. The program stops to ask you to start and stop it
G0 Z5
; Tool: Fine pen 0.5 mm
; Pen: the spindle stays off
; Operation 2: draw with Fine pen 0.5 mm, pen down at 0 mm below Z0, up at 2 mm above it
G0 X25 Y5
; Pass 1
G1 Z0 F300
G1 X35 F1500
G1 Y15
G1 X25
G1 Y5
G0 Z2
G0 Z5
G0 X0 Y0
; End of program