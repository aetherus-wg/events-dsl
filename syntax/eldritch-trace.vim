" Vim syntax file
" Language:    Eldritch-Trace
" Filenames:   *.ds
" Maintainer:  Cristian Bourceanu <bourceanu.cristi@gmail.com>
" Last Change: 2026 Apr 21

" quit when a syntax file was already loaded
if exists("b:current_syntax")
	finish
endif

syn case match

" Fields
syn keyword filterdslField MCRT
syn keyword filterdslField Emission
syn keyword filterdslField Detection
syn keyword filterdslField Material
syn keyword filterdslField Interface
syn keyword filterdslField Reflector
syn keyword filterdslField Elastic
syn keyword filterdslField Inelastic
syn keyword filterdslField Absorption
syn keyword filterdslField Mie
syn keyword filterdslField Rayleigh
syn keyword filterdslField HenyeyGreenstein
syn keyword filterdslField Raman
syn keyword filterdslField Fluorescence
syn keyword filterdslField Forward
syn keyword filterdslField Backward
syn keyword filterdslField Side
syn keyword filterdslField Unknown
syn keyword filterdslField Refraction
syn keyword filterdslField Reflection
syn keyword filterdslField ReEmittance
syn keyword filterdslField Diffuse
syn keyword filterdslField Specular
syn keyword filterdslField Composite
syn keyword filterdslField SphericalCDF

" Construction
syn keyword filterdslSet any
syn keyword filterdslSet seq
syn keyword filterdslSet perm

" SrcId constructors
syn match filterdslSrcId /\<Mat\ze\s*(/
syn match filterdslSrcId /\<MatSurf\ze\s*(/
syn match filterdslSrcId /\<Surf\ze\s*(/
syn match filterdslSrcId /\<Light\ze\s*(/
syn match filterdslSrcId /\<Detector\ze\s*(/

" Declaration
syn keyword filterdslDecl ledger
syn keyword filterdslDecl signals
syn keyword filterdslDecl src
syn keyword filterdslDecl pattern
syn keyword filterdslDecl sequence
syn keyword filterdslDecl rule

"syn match filterdsl "|"

" Special chars
syn match filterdslKeyChar  "="
syn match filterdslKeyChar  "+"
syn match filterdslKeyChar  "*"
syn match filterdslKeyChar  "?"
syn match filterdslKeyChar  "!"

syn match filterdslConcat "|"

" Special 'X' token
syn match filterdslDontCare /\<X\>/

" Errors
syn match filterdslBrackErr   "]"
syn match filterdslBraceErr   "}"

" Identifiers (excluding keywords)
" TODO: Find out how to get identifier match without overriding Mat/MatSurf...
"syn match filterdslIdent /\<[A-Za-z_][A-Za-z0-9_]*\>/
"syn match filterdslIdent /\<src\s\+\zs[A-Za-z_][A-Za-z0-9_]*\ze\s*=/
"syn match filterdslIdent /\<src\s\+\w\+\ze\s*=/
"syn region filterdslAssign start=/\<src\s\+/ end=/\s*=/ keepend contains=filterdslIdent
"syn match filterdslIdent /\<[A-Za-z_][A-Za-z0-9_]*\>/ contained

" Enclosing delimiters
syn region filterdslEncl transparent matchgroup=filterdslBrackEncl start="\[" matchgroup=filterdslBrackEncl end="\]" contains=ALLBUT,filterdslBrackErr
syn region filterdslEncl transparent matchgroup=filterdslBraceEncl start="{" matchgroup=filterdslBraceEncl end="}" contains=ALLBUT,filterdslBraceErr

" Comments
syn region filterdslComment start="#" skip="\\$" end="\n" keepend contains=filterdslComment,filterdslTodo

" Todo
syn keyword filterdslTodo TODO FIXME todo fixme Todo Fixme

" Strings
syntax region filterdslString start=/"/ end=/"/


" Numbers (decimal and hex)
syn match filterdslNumber /\<0x[0-9A-Fa-f]\+\>/
syn match filterdslNumber /\<\d\+\>/

hi def link filterdslParErr Error
hi def link filterdslBraceErr Error
hi def link filterdslBrackErr Error

hi def link filterdslComment Comment
hi def link filterdslTodo Todo

hi def link filterdslString String
hi def link filterdslNumber Number

hi def link filterdslField Type
hi def link filterdslKeyChar Operator

hi def link filterdslConcat Special
hi def link filterdslDontCare Special

hi def link filterdslSrcId Function
hi def link filterdslSet Statement
hi def link filterdslDecl Keyword

"hi def link filterdslIdent Identifier
"hi def link filterdslAssign Normal

let b:current_syntax = "Eldritch-Trace"
