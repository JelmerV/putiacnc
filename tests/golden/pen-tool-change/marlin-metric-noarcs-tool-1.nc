(Summary, written by PutiaCNC)
(Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
(                        speed shown, and again before a tool change and at the end to stop it.)
(                        There is no work coordinate system: jog to the zero of the job and set)
(                        it with G92 X0 Y0 Z0 first.)
(Tools:                  3 mm flat end mill. No tool change.)
(Pauses:                 The program stops 2 times with the spindle off: 'Start the spindle at)
(                        12000 rpm, then resume', 'Stop the spindle, then resume'. Resume it)
(                        when you are ready; the spindle starts again by itself.)
(Cutting:                Down to Z -1 mm. 61 mm of cutting, about 7 s in all.)
(End:                    Lifts to Z 5 mm, goes to X0 Y0.)
(File 1 of 3: 3 mm flat end mill operation 1)
; PutiaCNC - Marlin (no spindle control)
; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
G21 G90
; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
; Spindle: by hand. The program stops to ask you to start and stop it
G0 Z5
; Tool: 3 mm flat end mill
M0 Start the spindle at 12000 rpm, then resume
; Operation 1: outside, tool 3 mm
G0 X20.344 Y3.54
G0 Z0.5
G1 Z0 F250
; Pass 1
G1 Z-1
G1 F800
G1 X20.669 Y3.657
G1 X20.959 Y3.846
G1 X21.197 Y4.096
G1 X21.372 Y4.395
G1 X21.475 Y4.725
G1 X21.5 Y5
G1 Y15
G1 X21.46 Y15.344
G1 X21.343 Y15.669
G1 X21.154 Y15.959
G1 X20.904 Y16.197
G1 X20.605 Y16.372
G1 X20.275 Y16.475
G1 X20 Y16.5
G1 X5
G1 X4.656 Y16.46
G1 X4.331 Y16.343
G1 X4.041 Y16.154
G1 X3.803 Y15.904
G1 X3.628 Y15.605
G1 X3.525 Y15.275
G1 X3.5 Y15
G1 Y5
G1 X3.54 Y4.656
G1 X3.657 Y4.331
G1 X3.846 Y4.041
G1 X4.096 Y3.803
G1 X4.395 Y3.628
G1 X4.725 Y3.525
G1 X5 Y3.5
G1 X20
G1 X20.344 Y3.54
G0 Z5
M0 Stop the spindle, then resume
G0 X0 Y0
; End of program
(After this file: Operation 1 done: resume when ready)